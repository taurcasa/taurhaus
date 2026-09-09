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
    if source.join("state/terminal").exists() {
        return move_with_terminal_inodes(&source, &target);
    }
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

#[cfg(unix)]
fn move_with_terminal_inodes(
    source: &Path,
    target: &Path,
) -> Result<TeamMoveStrategy, CoordinationError> {
    let locks = fs::canonicalize(source.join("state/terminal"))?;
    if target.join("config.json").exists() {
        return Err(CoordinationError::Conflict(
            "target team already exists".into(),
        ));
    }
    let target_locks = target.join("state/terminal");
    let parent = target
        .parent()
        .ok_or_else(|| CoordinationError::Validation("team root missing".into()))?;
    fs::create_dir_all(parent)?;
    let staged = parent.join(format!(".terminal-move-{}", uuid::Uuid::new_v4()));
    let publish = (|| -> Result<(), CoordinationError> {
        let expected = snapshot_tree(source)?;
        copy_tree(source, &staged)?;
        verify_tree(&staged, &expected)?;
        fs::create_dir_all(target.join("state"))?;
        fs::create_dir_all(&target_locks)?;
        for entry in fs::read_dir(&locks)? {
            let entry = entry?;
            if entry.path().extension().is_none_or(|ext| ext != "lock") {
                continue;
            }
            let destination = target_locks.join(entry.file_name());
            match fs::hard_link(entry.path(), &destination) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                    use std::os::unix::fs::MetadataExt;
                    let source = entry.metadata()?;
                    let target = fs::metadata(destination)?;
                    if (source.dev(), source.ino()) != (target.dev(), target.ino()) {
                        return Err(CoordinationError::Conflict(
                            "target terminal inode authority differs".into(),
                        ));
                    }
                }
                Err(error) => {
                    return Err(CoordinationError::Validation(format!(
                    "terminal relocation requires same-volume hard links; source retained: {error}"
                )))
                }
            }
        }
        // Publish config last. The registry still binds the source until this
        // operation returns; rollback retains its complete payload on error.
        for entry in fs::read_dir(&staged)? {
            let entry = entry?;
            if entry.file_name() == "config.json" {
                continue;
            }
            if entry.file_name() == "state" {
                for child in fs::read_dir(entry.path())? {
                    let child = child?;
                    fs::rename(child.path(), target.join("state").join(child.file_name()))?;
                }
            } else {
                fs::rename(entry.path(), target.join(entry.file_name()))?;
            }
        }
        fs::rename(staged.join("config.json"), target.join("config.json"))?;
        Ok(())
    })();
    remove_dir_if_present(&staged);
    if let Err(error) = publish {
        let _ = crate::coordination::stores::config::remove_team_payload(target);
        return Err(error);
    }
    crate::coordination::stores::config::remove_team_payload(source)?;
    Ok(TeamMoveStrategy::CopyVerify)
}
#[cfg(not(unix))]
fn move_with_terminal_inodes(
    _source: &Path,
    _target: &Path,
) -> Result<TeamMoveStrategy, CoordinationError> {
    Err(CoordinationError::Validation(
        "terminal lock relocation requires the native daemon".into(),
    ))
}
fn terminal_entry(entry: &fs::DirEntry) -> bool {
    entry.file_name() == "terminal"
        && entry
            .path()
            .parent()
            .is_some_and(|p| p.file_name().is_some_and(|n| n == "state"))
}

fn copy_tree(source: &Path, target: &Path) -> Result<(), CoordinationError> {
    fs::create_dir(target)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        if terminal_entry(&entry) {
            continue;
        }
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
            if terminal_entry(&entry) {
                continue;
            }
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

    #[cfg(unix)]
    #[test]
    fn terminal_inode_survives_relocation_cycle_until_disband() {
        // Regression: 1127823e linked the target to its old account root and
        // retained a ghost directory even after final disband.
        use std::os::unix::fs::MetadataExt;
        let tmp = tempfile::TempDir::new().unwrap();
        let a = tmp.path().join("a/teams");
        let b = tmp.path().join("b/teams");
        let original = a.join("team/state/terminal/seat.lock");
        fs::create_dir_all(original.parent().unwrap()).unwrap();
        fs::write(&original, "").unwrap();
        fs::write(a.join("team/config.json"), "{}").unwrap();
        let inode = fs::metadata(&original).unwrap().ino();
        move_team_directory(&a, &b, "team").unwrap();
        assert_eq!(fs::metadata(&original).unwrap().ino(), inode);
        assert_eq!(
            fs::metadata(b.join("team/state/terminal/seat.lock"))
                .unwrap()
                .ino(),
            inode
        );
        fs::remove_dir_all(tmp.path().join("a")).unwrap();
        assert_eq!(
            fs::metadata(b.join("team/state/terminal/seat.lock"))
                .unwrap()
                .ino(),
            inode
        );
        move_team_directory(&b, &a, "team").unwrap();
        assert!(crate::coordination::stores::TeamConfigStore::list(&b)
            .unwrap()
            .is_empty());
        assert_eq!(fs::metadata(&original).unwrap().ino(), inode);
        crate::coordination::stores::TeamConfigStore::delete(&a, "team").unwrap();
        assert!(!a.join("team").exists());
    }

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

    #[test]
    fn cross_device_copy_move_verifies_promotes_and_removes_temporaries() {
        let temp = tempfile::TempDir::new().expect("tempdir");
        let source = temp.path().join("default/teams");
        let target = temp.path().join("work/teams");
        std::fs::create_dir_all(source.join("arch/state")).expect("source tree");
        std::fs::write(source.join("arch/config.json"), b"config").expect("config");
        std::fs::write(source.join("arch/state/runtime.json"), b"runtime").expect("runtime");
        let direct_source = source.join("arch");
        let direct_target = target.join("arch");
        let mut cross_device_once = |from: &Path, to: &Path| {
            if from == direct_source && to == direct_target {
                Err(std::io::Error::from(std::io::ErrorKind::CrossesDevices))
            } else {
                std::fs::rename(from, to)
            }
        };

        let strategy = move_team_directory_with(&source, &target, "arch", &mut cross_device_once)
            .expect("copy+verify move");

        assert_eq!(strategy, TeamMoveStrategy::CopyVerify);
        assert!(!source.join("arch").exists());
        assert_eq!(
            std::fs::read(target.join("arch/state/runtime.json")).expect("moved runtime"),
            b"runtime"
        );
        for root in [&source, &target] {
            let leftovers = std::fs::read_dir(root)
                .expect("teams root")
                .filter_map(Result::ok)
                .map(|entry| entry.file_name().to_string_lossy().into_owned())
                .filter(|name| name.starts_with(".arch.taurhaus-"))
                .collect::<Vec<_>>();
            assert!(leftovers.is_empty(), "leftover move paths: {leftovers:?}");
        }
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
    #[test]
    fn recovery_root_move_and_rollback_preserve_recipient_and_fence_stale_receipts() {
        use crate::coordination::recovery_card::{DeliveryKind, ReceiptStage};
        use crate::coordination::recovery_delivery::{
            attach_receipt, observe, prepare, reserve_activation,
        };
        use crate::coordination::stores::{MeshInboxMessage, MeshInboxStore, TeamRootRegistry};
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("account/teams");
        fs::create_dir_all(root.join("team")).unwrap();
        fs::write(root.join("team/config.json"),serde_json::json!({"schema_version":1,"name":"team","created_at":chrono::Utc::now(),"team_incarnation_id":"team-1","members":[{"name":"seat","role":"agent","cli_tool":"codex","project_path":temp.path()}]}).to_string()).unwrap();
        fs::create_dir_all(root.join("team/runtime")).unwrap();
        fs::write(
            root.join("team/runtime/seat.json"),
            r#"{"member_name":"seat","session_id":"session-1"}"#,
        )
        .unwrap();
        let registry = TeamRootRegistry::new(root.clone());
        reserve_activation(&root, "team", "seat", "activation").unwrap();
        let first = prepare(&registry, &root, "team", "seat", "inbox")
            .unwrap()
            .unwrap();
        let mut message =
            MeshInboxMessage::new("lead", first.text.clone(), None, chrono::Utc::now());
        attach_receipt(&mut message, Some(&first.receipt));
        MeshInboxStore::append(&root, "team", "seat", &message).unwrap();
        observe(
            &registry,
            &root,
            "team",
            "seat",
            &first.receipt,
            ReceiptStage::Accepted,
        )
        .unwrap();
        let target = temp.path().join("other/teams");
        crate::daemon::team_move::move_team_directory(&root, &target, "team").unwrap();
        registry.set("team", &target).unwrap();
        assert!(prepare(&registry, &root, "team", "seat", "inbox").is_err());
        assert!(observe(
            &registry,
            &target,
            "team",
            "seat",
            &first.receipt,
            ReceiptStage::Accepted
        )
        .is_err());
        let moved = prepare(&registry, &target, "team", "seat", "inbox")
            .unwrap()
            .unwrap();
        assert_eq!(moved.receipt.obligation_key, first.receipt.obligation_key);
        assert_eq!(moved.receipt.kind, DeliveryKind::Correction);
        assert_eq!(
            moved.receipt.supersedes_revision.as_ref(),
            Some(&first.receipt.content_revision)
        );
        crate::daemon::team_move::move_team_directory(&target, &root, "team").unwrap();
        registry.set("team", &root).unwrap();
        let rollback = prepare(&registry, &root, "team", "seat", "inbox")
            .unwrap()
            .unwrap();
        assert_ne!(rollback.receipt.delivery_id, first.receipt.delivery_id);
        assert_eq!(rollback.receipt.kind, DeliveryKind::Correction);
    }
}
