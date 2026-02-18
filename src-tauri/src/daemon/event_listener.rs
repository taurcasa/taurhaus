use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Duration;

use crate::daemon::protocol::{self, DaemonMessage, DaemonRequest, DaemonResponse};
use crate::errors::AppError;
use crate::fs::watcher::WatchEvent;

/// Listens for push events from the daemon over a dedicated TCP connection.
///
/// Opens a separate connection from the main `DaemonProvider` (which is used
/// for request/response operations). Sends `watch` commands for WSL projects,
/// then reads events in a blocking loop, converting daemon events to
/// `WatchEvent`s and forwarding them to the app's event channel.
pub struct DaemonEventListener {
    stream: TcpStream,
    reader: BufReader<TcpStream>,
    event_tx: mpsc::Sender<WatchEvent>,
    /// Mapping from watched Linux path → project_id.
    path_to_project: HashMap<String, String>,
    next_id: u64,
}

impl DaemonEventListener {
    /// Connect to the daemon for event listening.
    pub fn connect(
        addr: &str,
        event_tx: mpsc::Sender<WatchEvent>,
    ) -> Result<Self, AppError> {
        let stream = TcpStream::connect(addr).map_err(|e| {
            AppError::Io(std::io::Error::new(
                e.kind(),
                format!("Event listener connect to {addr} failed: {e}"),
            ))
        })?;
        let reader = BufReader::new(stream.try_clone().map_err(AppError::Io)?);

        Ok(Self {
            stream,
            reader,
            event_tx,
            path_to_project: HashMap::new(),
            next_id: 1,
        })
    }

    /// Send a watch command for a project. The daemon will start watching the
    /// directory and push events back on this connection.
    pub fn watch(&mut self, project_id: &str, linux_path: &str) -> Result<(), AppError> {
        let id = format!("ew{}", self.next_id);
        self.next_id += 1;

        let request = DaemonRequest::new(
            &id,
            protocol::method::WATCH,
            protocol::PathParams {
                path: linux_path.to_string(),
            },
        );

        let json = serde_json::to_string(&request).map_err(|e| {
            AppError::InvalidPath(format!("Serialize watch request failed: {e}"))
        })?;
        self.stream.write_all(json.as_bytes()).map_err(AppError::Io)?;
        self.stream.write_all(b"\n").map_err(AppError::Io)?;
        self.stream.flush().map_err(AppError::Io)?;

        // Read lines until we get the watch response. The daemon may push events
        // on this connection before the response arrives (e.g. if a previously
        // watched project fires an event between our request and its response).
        let mut line = String::new();
        let response: DaemonResponse = loop {
            line.clear();
            self.reader.read_line(&mut line).map_err(AppError::Io)?;
            match serde_json::from_str::<DaemonMessage>(line.trim()) {
                Ok(DaemonMessage::Response(r)) => break r,
                Ok(DaemonMessage::Event(event)) => {
                    // Forward the event — it's from a previously registered watch
                    self.handle_event(event);
                }
                Err(e) => {
                    return Err(AppError::InvalidPath(format!(
                        "Parse watch response failed: {e}"
                    )));
                }
            }
        };

        if !response.is_ok() {
            return Err(AppError::InvalidPath(format!(
                "Daemon watch failed: {}",
                response
                    .error
                    .map(|e| e.message)
                    .unwrap_or_else(|| "unknown".into())
            )));
        }

        self.path_to_project
            .insert(linux_path.to_string(), project_id.to_string());
        tracing::info!(project_id, linux_path, "Daemon watch registered");
        Ok(())
    }

    /// Run the event loop. Blocks until the connection drops.
    ///
    /// Call this on a background thread after all `watch()` calls are done.
    /// Events are forwarded to the `event_tx` channel as `WatchEvent`s.
    pub fn run(mut self) {
        let _ = self
            .stream
            .set_read_timeout(Some(Duration::from_secs(5)));

        let mut line = String::new();
        loop {
            line.clear();
            match self.reader.read_line(&mut line) {
                Ok(0) => break, // Connection closed
                Ok(_) => self.handle_line(&line),
                Err(ref e)
                    if e.kind() == std::io::ErrorKind::TimedOut
                        || e.kind() == std::io::ErrorKind::WouldBlock =>
                {
                    continue;
                }
                Err(_) => break,
            }
        }

        tracing::info!("Daemon event listener disconnected");
    }

    fn handle_line(&self, line: &str) {
        let line = line.trim();
        if line.is_empty() {
            return;
        }

        let msg: DaemonMessage = match serde_json::from_str(line) {
            Ok(m) => m,
            Err(e) => {
                tracing::debug!("Event listener: unparseable line: {e}");
                return;
            }
        };

        match msg {
            DaemonMessage::Event(event) => self.handle_event(event),
            DaemonMessage::Response(_) => {
                // Stray response — ignore (watch responses are handled synchronously)
            }
        }
    }

    fn handle_event(&self, event: protocol::DaemonEvent) {
        if let Some(watch_event) =
            convert_daemon_event(event, &self.path_to_project)
        {
            let _ = self.event_tx.send(watch_event);
        }
    }
}

/// Convert a daemon push event into a `WatchEvent`, using the path→project_id
/// mapping to tag events with the correct project.
///
/// Returns `None` if the event's path doesn't match any watched project.
fn convert_daemon_event(
    event: protocol::DaemonEvent,
    path_to_project: &HashMap<String, String>,
) -> Option<WatchEvent> {
    match event.event.as_str() {
        protocol::event::GIT_CHANGED => {
            let data: protocol::GitChangedData =
                serde_json::from_value(event.data).ok()?;
            let project_id = path_to_project.get(&data.path)?;
            Some(WatchEvent::GitChanged {
                project_id: project_id.clone(),
            })
        }
        protocol::event::FILE_CHANGED => {
            let data: protocol::FileChangedData =
                serde_json::from_value(event.data).ok()?;
            let project_id = path_to_project.get(&data.path)?;
            let project_root = PathBuf::from(&data.path);
            let paths: Vec<PathBuf> =
                data.files.iter().map(|f| project_root.join(f)).collect();
            Some(WatchEvent::FileChanged {
                project_id: project_id.clone(),
                paths,
            })
        }
        protocol::event::SESSION_FILE_CREATED => {
            let data: protocol::SessionFileCreatedData =
                serde_json::from_value(event.data).ok()?;
            let project_id = path_to_project.get(&data.path)?;
            let full_path = PathBuf::from(&data.path).join(&data.file);
            Some(WatchEvent::SessionFileCreated {
                project_id: project_id.clone(),
                path: full_path,
            })
        }
        _ => {
            tracing::debug!(event = event.event, "Unknown daemon event type");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::daemon::server::DaemonConfig;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    fn start_daemon() -> (u16, Arc<AtomicBool>) {
        let shutdown = Arc::new(AtomicBool::new(false));
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);

        let config = DaemonConfig {
            port,
            bind_addr: "127.0.0.1".to_string(),
            idle_timeout_secs: None,
        };
        let shutdown_clone = shutdown.clone();
        std::thread::spawn(move || {
            let _ = crate::daemon::server::run(&config, shutdown_clone);
        });
        std::thread::sleep(Duration::from_millis(100));
        (port, shutdown)
    }

    #[test]
    fn event_listener_connects_and_watches() {
        let (port, shutdown) = start_daemon();
        let dir = tempfile::TempDir::new().unwrap();
        let (tx, _rx) = mpsc::channel();

        let mut listener =
            DaemonEventListener::connect(&format!("127.0.0.1:{port}"), tx).unwrap();
        let result = listener.watch("p1", dir.path().to_str().unwrap());
        assert!(result.is_ok());

        shutdown.store(true, Ordering::Relaxed);
    }

    #[test]
    fn event_listener_watch_nonexistent_fails() {
        let (port, shutdown) = start_daemon();
        let (tx, _rx) = mpsc::channel();

        let mut listener =
            DaemonEventListener::connect(&format!("127.0.0.1:{port}"), tx).unwrap();
        let result = listener.watch("p1", "/nonexistent/path/that/does/not/exist");
        assert!(result.is_err());

        shutdown.store(true, Ordering::Relaxed);
    }

    #[test]
    fn event_listener_receives_file_change() {
        let (port, shutdown) = start_daemon();
        let dir = tempfile::TempDir::new().unwrap();
        let (tx, rx) = mpsc::channel();

        let mut listener =
            DaemonEventListener::connect(&format!("127.0.0.1:{port}"), tx).unwrap();
        listener
            .watch("p1", dir.path().to_str().unwrap())
            .unwrap();

        // Start the event loop on a background thread
        let listener_handle = std::thread::spawn(move || {
            listener.run();
        });

        // Create a file in the watched directory
        std::thread::sleep(Duration::from_millis(200));
        std::fs::write(dir.path().join("test.txt"), "hello").unwrap();

        // Wait for the event (notify can take a moment)
        let event = rx.recv_timeout(Duration::from_secs(5));
        assert!(
            event.is_ok(),
            "Should receive a file change event within 5s"
        );

        let event = event.unwrap();
        match event {
            WatchEvent::FileChanged { project_id, paths } => {
                assert_eq!(project_id, "p1");
                assert!(!paths.is_empty());
            }
            other => {
                // Depending on timing, we might get a GitChanged first if .git exists
                // That's also acceptable
                tracing::debug!("Received: {other:?}");
            }
        }

        shutdown.store(true, Ordering::Relaxed);
        // Event loop will exit when daemon connection drops
        let _ = listener_handle.join();
    }

    #[test]
    fn event_listener_receives_git_change() {
        let (port, shutdown) = start_daemon();
        let dir = tempfile::TempDir::new().unwrap();

        // Create a git repo structure so .git/HEAD changes are classified as GitInternal
        let git_dir = dir.path().join(".git");
        std::fs::create_dir_all(git_dir.join("refs/heads")).unwrap();
        std::fs::write(git_dir.join("HEAD"), "ref: refs/heads/main").unwrap();

        let (tx, rx) = mpsc::channel();
        let mut listener =
            DaemonEventListener::connect(&format!("127.0.0.1:{port}"), tx).unwrap();
        listener
            .watch("p1", dir.path().to_str().unwrap())
            .unwrap();

        let listener_handle = std::thread::spawn(move || {
            listener.run();
        });

        // Modify a git internal file
        std::thread::sleep(Duration::from_millis(200));
        std::fs::write(git_dir.join("HEAD"), "ref: refs/heads/feature").unwrap();

        // Collect events for up to 5 seconds
        let mut got_git = false;
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while std::time::Instant::now() < deadline {
            match rx.recv_timeout(Duration::from_millis(200)) {
                Ok(WatchEvent::GitChanged { project_id }) => {
                    assert_eq!(project_id, "p1");
                    got_git = true;
                    break;
                }
                Ok(_) => continue,
                Err(_) => continue,
            }
        }
        assert!(got_git, "Should receive a GitChanged event");

        shutdown.store(true, Ordering::Relaxed);
        let _ = listener_handle.join();
    }

    #[test]
    fn convert_git_changed_event() {
        let mut mapping = HashMap::new();
        mapping.insert("/home/user/project".to_string(), "proj-1".to_string());

        let event = protocol::DaemonEvent::new(
            protocol::event::GIT_CHANGED,
            protocol::GitChangedData {
                path: "/home/user/project".to_string(),
            },
        );
        let result = convert_daemon_event(event, &mapping);
        assert!(matches!(
            result,
            Some(WatchEvent::GitChanged { ref project_id }) if project_id == "proj-1"
        ));
    }

    #[test]
    fn convert_file_changed_event() {
        let mut mapping = HashMap::new();
        mapping.insert("/home/user/project".to_string(), "proj-1".to_string());

        let event = protocol::DaemonEvent::new(
            protocol::event::FILE_CHANGED,
            protocol::FileChangedData {
                path: "/home/user/project".to_string(),
                files: vec!["src/main.rs".to_string()],
            },
        );
        let result = convert_daemon_event(event, &mapping);
        match result {
            Some(WatchEvent::FileChanged { project_id, paths }) => {
                assert_eq!(project_id, "proj-1");
                assert_eq!(paths.len(), 1);
                assert!(paths[0].to_string_lossy().contains("src/main.rs"));
            }
            other => panic!("Expected FileChanged, got: {other:?}"),
        }
    }

    #[test]
    fn convert_session_file_created_event() {
        let mut mapping = HashMap::new();
        mapping.insert("/home/user/project".to_string(), "proj-1".to_string());

        let event = protocol::DaemonEvent::new(
            protocol::event::SESSION_FILE_CREATED,
            protocol::SessionFileCreatedData {
                path: "/home/user/project".to_string(),
                file: "docs/sessions/session-2026-01-15.md".to_string(),
            },
        );
        let result = convert_daemon_event(event, &mapping);
        match result {
            Some(WatchEvent::SessionFileCreated { project_id, path }) => {
                assert_eq!(project_id, "proj-1");
                assert!(path.to_string_lossy().contains("session-2026-01-15.md"));
            }
            other => panic!("Expected SessionFileCreated, got: {other:?}"),
        }
    }

    #[test]
    fn convert_event_unknown_path_returns_none() {
        let mapping = HashMap::new(); // empty — no projects registered

        let event = protocol::DaemonEvent::new(
            protocol::event::GIT_CHANGED,
            protocol::GitChangedData {
                path: "/unknown/path".to_string(),
            },
        );
        assert!(convert_daemon_event(event, &mapping).is_none());
    }

    #[test]
    fn convert_unknown_event_type_returns_none() {
        let mut mapping = HashMap::new();
        mapping.insert("/home/user/project".to_string(), "proj-1".to_string());

        let event = protocol::DaemonEvent {
            event: "unknown_event".to_string(),
            data: serde_json::json!({"path": "/home/user/project"}),
        };
        assert!(convert_daemon_event(event, &mapping).is_none());
    }
}
