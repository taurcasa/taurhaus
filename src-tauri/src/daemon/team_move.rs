use std::collections::BTreeMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::coordination::errors::CoordinationError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TeamMoveStrategy {
    Rename,
    CopyVerify,
}

/// Relocate one complete team directory while keeping exactly one loadable
/// `<teams>/<name>` path at every failure boundary.
pub(crate) fn move_team_directory(
    source_teams: &Path,
    target_teams: &Path,
    team_name: &str,
) -> Result<TeamMoveStrategy, CoordinationError> {
    move_team_directory_with(source_teams, target_teams, team_name, &mut |from, to| {
        fs::rename(from, to)
    })
}

fn move_team_directory_with(
    source_teams: &Path,
    target_teams: &Path,
    team_name: &str,
    rename: &mut impl FnMut(&Path, &Path) -> std::io::Result<()>,
) -> Result<TeamMoveStrategy, CoordinationError> {
    let source = source_teams.join(team_name);
    let target = target_teams.join(team_name);
    if !source.is_dir() {
        return Err(CoordinationError::NotFound(format!(
            "team directory not found at '{}'",
            source.display()
        )));
    }
    if target.exists() {
        return Err(CoordinationError::Validation(format!(
            "target team directory already exists at '{}'",
            target.display()
        )));
    }
    fs::create_dir_all(target_teams)?;
    match rename(&source, &target) {
        Ok(()) => return Ok(TeamMoveStrategy::Rename),
        Err(error) if error.kind() == std::io::ErrorKind::CrossesDevices => {}
        Err(error) => return Err(CoordinationError::Io(error)),
    }

    let nonce = uuid::Uuid::new_v4().simple().to_string();
    let staging = target_teams.join(format!(".{team_name}.taurhaus-move-{nonce}"));
    let backup = source_teams.join(format!(".{team_name}.taurhaus-backup-{nonce}"));
    let expected = snapshot_tree(&source)?;
    if let Err(error) = copy_tree(&source, &staging).and_then(|()| verify_tree(&staging, &expected))
    {
        remove_dir_if_present(&staging);
        return Err(error);
    }

    if let Err(error) = rename(&source, &backup) {
        remove_dir_if_present(&staging);
        return Err(CoordinationError::Io(error));
    }
    if let Err(error) = rename(&staging, &target) {
        let restore = rename(&backup, &source);
        remove_dir_if_present(&staging);
        return match restore {
            Ok(()) => Err(CoordinationError::Io(error)),
            Err(restore_error) => Err(CoordinationError::StoreError(format!(
                "team move promotion failed ({error}); source restore failed ({restore_error})"
            ))),
        };
    }

    if let Err(error) = fs::remove_dir_all(&backup) {
        tracing::warn!(
            path = %backup.display(),
            error = %error,
            "verified team move left a hidden source backup"
        );
    }
    Ok(TeamMoveStrategy::CopyVerify)
}

fn copy_tree(source: &Path, target: &Path) -> Result<(), CoordinationError> {
    fs::create_dir(target)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let destination = target.join(entry.file_name());
        if file_type.is_dir() {
            copy_tree(&entry.path(), &destination)?;
        } else if file_type.is_file() {
            fs::copy(entry.path(), destination)?;
        } else {
            return Err(CoordinationError::Validation(format!(
                "team state contains unsupported filesystem entry '{}'",
                entry.path().display()
            )));
        }
    }
    Ok(())
}

fn snapshot_tree(root: &Path) -> Result<BTreeMap<PathBuf, (u64, String)>, CoordinationError> {
    fn visit(
        root: &Path,
        current: &Path,
        snapshot: &mut BTreeMap<PathBuf, (u64, String)>,
    ) -> Result<(), CoordinationError> {
        for entry in fs::read_dir(current)? {
            let entry = entry?;
            let file_type = entry.file_type()?;
            if file_type.is_dir() {
                visit(root, &entry.path(), snapshot)?;
            } else if file_type.is_file() {
                let relative = entry
                    .path()
                    .strip_prefix(root)
                    .map_err(|error| CoordinationError::StoreError(error.to_string()))?
                    .to_path_buf();
                snapshot.insert(relative, hash_file(&entry.path())?);
            } else {
                return Err(CoordinationError::Validation(format!(
                    "team state contains unsupported filesystem entry '{}'",
                    entry.path().display()
                )));
            }
        }
        Ok(())
    }

    let mut snapshot = BTreeMap::new();
    visit(root, root, &mut snapshot)?;
    Ok(snapshot)
}

fn verify_tree(
    root: &Path,
    expected: &BTreeMap<PathBuf, (u64, String)>,
) -> Result<(), CoordinationError> {
    let actual = snapshot_tree(root)?;
    if &actual != expected {
        return Err(CoordinationError::StoreError(format!(
            "copied team state failed verification at '{}'",
            root.display()
        )));
    }
    Ok(())
}

fn hash_file(path: &Path) -> Result<(u64, String), CoordinationError> {
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut length = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
        length += read as u64;
    }
    Ok((length, format!("{:x}", hasher.finalize())))
}

fn remove_dir_if_present(path: &Path) {
    if path.exists() {
        let _ = fs::remove_dir_all(path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rename_move_preserves_the_complete_team_tree() {
        let temp = tempfile::TempDir::new().expect("tempdir");
        let source = temp.path().join("default/teams");
        let target = temp.path().join("work/teams");
        std::fs::create_dir_all(source.join("arch/state")).expect("source tree");
        std::fs::write(source.join("arch/config.json"), b"config").expect("config");
        std::fs::write(source.join("arch/state/runtime.json"), b"runtime").expect("runtime");

        let strategy = move_team_directory(&source, &target, "arch").expect("move team");

        assert_eq!(strategy, TeamMoveStrategy::Rename);
        assert!(!source.join("arch").exists());
        assert_eq!(
            std::fs::read(target.join("arch/state/runtime.json")).expect("moved runtime"),
            b"runtime"
        );
    }

    #[cfg(unix)]
    #[test]
    fn failed_copy_move_leaves_only_the_source_team_loadable() {
        use std::os::unix::fs::symlink;

        let temp = tempfile::TempDir::new().expect("tempdir");
        let source = temp.path().join("default/teams");
        let target = temp.path().join("work/teams");
        std::fs::create_dir_all(source.join("arch")).expect("source tree");
        std::fs::write(source.join("arch/config.json"), b"config").expect("config");
        symlink("config.json", source.join("arch/unsupported-link")).expect("symlink");
        let mut force_cross_device = |_from: &Path, _to: &Path| {
            Err(std::io::Error::from(std::io::ErrorKind::CrossesDevices))
        };

        let error = move_team_directory_with(&source, &target, "arch", &mut force_cross_device)
            .expect_err("unsupported copy entry must fail");

        assert!(error.to_string().contains("unsupported filesystem entry"));
        assert!(source.join("arch/config.json").is_file());
        assert!(!target.join("arch").exists());
    }
}
