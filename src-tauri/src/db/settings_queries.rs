//! Settings persistence via SQLite key-value store.
//!
//! Settings are stored as individual key-value pairs in the `settings` table.
//! The `Settings` struct is serialized/deserialized to/from these pairs.

use rusqlite::Connection;

use crate::models::{ActivityThresholds, DaemonSettings, Settings};

/// Get a single setting value by key.
pub fn get_setting(conn: &Connection, key: &str) -> Result<Option<String>, rusqlite::Error> {
    conn.query_row(
        "SELECT value FROM settings WHERE key = ?1",
        [key],
        |row| row.get(0),
    )
    .optional()
}

/// Set a single setting value (insert or update).
pub fn set_setting(conn: &Connection, key: &str, value: &str) -> Result<(), rusqlite::Error> {
    conn.execute(
        "INSERT INTO settings (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        rusqlite::params![key, value],
    )?;
    Ok(())
}

/// Delete a single setting.
pub fn delete_setting(conn: &Connection, key: &str) -> Result<bool, rusqlite::Error> {
    let rows = conn.execute("DELETE FROM settings WHERE key = ?1", [key])?;
    Ok(rows > 0)
}

// Key constants
const KEY_SCAN_DIRS: &str = "scan_directories";
const KEY_ACTIVE_DAYS: &str = "thresholds.active_days";
const KEY_RECENT_DAYS: &str = "thresholds.recent_days";
const KEY_STALE_DAYS: &str = "thresholds.stale_days";
const KEY_IGNORE_PATTERNS: &str = "ignore_patterns";
const KEY_DAEMON_PORT: &str = "daemon.port";
const KEY_DAEMON_PATH: &str = "daemon.path";
const KEY_DAEMON_AUTO_START: &str = "daemon.auto_start";

/// Load all settings from the database, falling back to defaults for missing keys.
pub fn get_all_settings(conn: &Connection) -> Result<Settings, rusqlite::Error> {
    let defaults = Settings::default();

    let scan_directories = get_setting(conn, KEY_SCAN_DIRS)?
        .and_then(|v| serde_json::from_str(&v).ok())
        .unwrap_or(defaults.scan_directories);

    let active_days = get_setting(conn, KEY_ACTIVE_DAYS)?
        .and_then(|v| v.parse().ok())
        .unwrap_or(defaults.thresholds.active_days);

    let recent_days = get_setting(conn, KEY_RECENT_DAYS)?
        .and_then(|v| v.parse().ok())
        .unwrap_or(defaults.thresholds.recent_days);

    let stale_days = get_setting(conn, KEY_STALE_DAYS)?
        .and_then(|v| v.parse().ok())
        .unwrap_or(defaults.thresholds.stale_days);

    let ignore_patterns = get_setting(conn, KEY_IGNORE_PATTERNS)?
        .and_then(|v| serde_json::from_str(&v).ok())
        .unwrap_or(defaults.ignore_patterns);

    let daemon_port = get_setting(conn, KEY_DAEMON_PORT)?
        .and_then(|v| v.parse().ok())
        .unwrap_or(defaults.daemon.port);

    let daemon_path = get_setting(conn, KEY_DAEMON_PATH)?
        .unwrap_or(defaults.daemon.path);

    let daemon_auto_start = get_setting(conn, KEY_DAEMON_AUTO_START)?
        .and_then(|v| v.parse().ok())
        .unwrap_or(defaults.daemon.auto_start);

    Ok(Settings {
        scan_directories,
        thresholds: ActivityThresholds {
            active_days,
            recent_days,
            stale_days,
        },
        ignore_patterns,
        daemon: DaemonSettings {
            port: daemon_port,
            path: daemon_path,
            auto_start: daemon_auto_start,
        },
    })
}

/// Save all settings to the database.
pub fn save_settings(conn: &Connection, settings: &Settings) -> Result<(), rusqlite::Error> {
    let scan_dirs_json =
        serde_json::to_string(&settings.scan_directories).unwrap_or_else(|_| "[]".to_string());
    set_setting(conn, KEY_SCAN_DIRS, &scan_dirs_json)?;

    set_setting(
        conn,
        KEY_ACTIVE_DAYS,
        &settings.thresholds.active_days.to_string(),
    )?;
    set_setting(
        conn,
        KEY_RECENT_DAYS,
        &settings.thresholds.recent_days.to_string(),
    )?;
    set_setting(
        conn,
        KEY_STALE_DAYS,
        &settings.thresholds.stale_days.to_string(),
    )?;

    let ignore_json =
        serde_json::to_string(&settings.ignore_patterns).unwrap_or_else(|_| "[]".to_string());
    set_setting(conn, KEY_IGNORE_PATTERNS, &ignore_json)?;

    set_setting(conn, KEY_DAEMON_PORT, &settings.daemon.port.to_string())?;
    set_setting(conn, KEY_DAEMON_PATH, &settings.daemon.path)?;
    set_setting(
        conn,
        KEY_DAEMON_AUTO_START,
        &settings.daemon.auto_start.to_string(),
    )?;

    Ok(())
}

// Make `optional()` available for query_row
use rusqlite::OptionalExtension;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::init_db;
    use tempfile::NamedTempFile;

    fn test_db() -> (Connection, NamedTempFile) {
        let tmp = NamedTempFile::new().unwrap();
        let conn = init_db(tmp.path()).unwrap();
        (conn, tmp)
    }

    #[test]
    fn get_nonexistent_setting_returns_none() {
        let (conn, _tmp) = test_db();
        let result = get_setting(&conn, "nonexistent").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn set_and_get_setting() {
        let (conn, _tmp) = test_db();
        set_setting(&conn, "test_key", "test_value").unwrap();
        let result = get_setting(&conn, "test_key").unwrap();
        assert_eq!(result, Some("test_value".to_string()));
    }

    #[test]
    fn set_setting_upserts() {
        let (conn, _tmp) = test_db();
        set_setting(&conn, "key", "value1").unwrap();
        set_setting(&conn, "key", "value2").unwrap();
        let result = get_setting(&conn, "key").unwrap();
        assert_eq!(result, Some("value2".to_string()));
    }

    #[test]
    fn delete_existing_setting() {
        let (conn, _tmp) = test_db();
        set_setting(&conn, "key", "value").unwrap();
        assert!(super::delete_setting(&conn, "key").unwrap());
        assert!(get_setting(&conn, "key").unwrap().is_none());
    }

    #[test]
    fn delete_nonexistent_returns_false() {
        let (conn, _tmp) = test_db();
        assert!(!super::delete_setting(&conn, "nonexistent").unwrap());
    }

    #[test]
    fn get_all_settings_returns_defaults_when_empty() {
        let (conn, _tmp) = test_db();
        let settings = get_all_settings(&conn).unwrap();
        let defaults = Settings::default();
        assert_eq!(settings.thresholds.active_days, defaults.thresholds.active_days);
        assert_eq!(settings.thresholds.recent_days, defaults.thresholds.recent_days);
        assert_eq!(settings.thresholds.stale_days, defaults.thresholds.stale_days);
        assert!(settings.scan_directories.is_empty());
        assert!(settings.ignore_patterns.is_empty());
    }

    #[test]
    fn save_and_load_settings_roundtrip() {
        let (conn, _tmp) = test_db();

        let settings = Settings {
            scan_directories: vec!["~/projects".to_string(), "~/work".to_string()],
            thresholds: ActivityThresholds {
                active_days: 5,
                recent_days: 14,
                stale_days: 60,
            },
            ignore_patterns: vec!["node_modules".to_string(), ".git".to_string()],
            daemon: DaemonSettings {
                port: 18000,
                path: "/custom/daemon".to_string(),
                auto_start: false,
            },
        };

        save_settings(&conn, &settings).unwrap();
        let loaded = get_all_settings(&conn).unwrap();

        assert_eq!(loaded.scan_directories, settings.scan_directories);
        assert_eq!(loaded.thresholds.active_days, 5);
        assert_eq!(loaded.thresholds.recent_days, 14);
        assert_eq!(loaded.thresholds.stale_days, 60);
        assert_eq!(loaded.ignore_patterns, settings.ignore_patterns);
        assert_eq!(loaded.daemon.port, 18000);
        assert_eq!(loaded.daemon.path, "/custom/daemon");
        assert!(!loaded.daemon.auto_start);
    }

    #[test]
    fn partial_settings_uses_defaults_for_missing() {
        let (conn, _tmp) = test_db();

        // Only set some values
        set_setting(&conn, "thresholds.active_days", "3").unwrap();

        let loaded = get_all_settings(&conn).unwrap();
        assert_eq!(loaded.thresholds.active_days, 3);
        // Others should be defaults
        assert_eq!(loaded.thresholds.recent_days, 30);
        assert_eq!(loaded.thresholds.stale_days, 90);
    }

    #[test]
    fn save_overwrites_previous_settings() {
        let (conn, _tmp) = test_db();

        let settings1 = Settings {
            scan_directories: vec!["~/old".to_string()],
            thresholds: ActivityThresholds {
                active_days: 7,
                recent_days: 30,
                stale_days: 90,
            },
            ignore_patterns: vec![],
            daemon: DaemonSettings::default(),
        };
        save_settings(&conn, &settings1).unwrap();

        let settings2 = Settings {
            scan_directories: vec!["~/new".to_string()],
            thresholds: ActivityThresholds {
                active_days: 3,
                recent_days: 14,
                stale_days: 60,
            },
            ignore_patterns: vec!["target".to_string()],
            daemon: DaemonSettings::default(),
        };
        save_settings(&conn, &settings2).unwrap();

        let loaded = get_all_settings(&conn).unwrap();
        assert_eq!(loaded.scan_directories, vec!["~/new".to_string()]);
        assert_eq!(loaded.thresholds.active_days, 3);
        assert_eq!(loaded.ignore_patterns, vec!["target".to_string()]);
    }

    #[test]
    fn invalid_stored_value_falls_back_to_default() {
        let (conn, _tmp) = test_db();

        // Store invalid JSON for scan_directories
        set_setting(&conn, "scan_directories", "not-json").unwrap();
        // Store invalid integer for threshold
        set_setting(&conn, "thresholds.active_days", "not-a-number").unwrap();

        let loaded = get_all_settings(&conn).unwrap();
        // Should fall back to defaults
        assert!(loaded.scan_directories.is_empty());
        assert_eq!(loaded.thresholds.active_days, 7);
    }

    #[test]
    fn daemon_settings_default_when_empty() {
        let (conn, _tmp) = test_db();
        let loaded = get_all_settings(&conn).unwrap();
        let defaults = DaemonSettings::default();
        assert_eq!(loaded.daemon.port, defaults.port);
        assert_eq!(loaded.daemon.path, defaults.path);
        assert_eq!(loaded.daemon.auto_start, defaults.auto_start);
    }

    #[test]
    fn daemon_settings_invalid_port_falls_back_to_default() {
        let (conn, _tmp) = test_db();
        set_setting(&conn, "daemon.port", "not-a-number").unwrap();
        let loaded = get_all_settings(&conn).unwrap();
        assert_eq!(loaded.daemon.port, DaemonSettings::default().port);
    }
}
