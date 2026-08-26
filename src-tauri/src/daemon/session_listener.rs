use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::time::Duration;

use crate::daemon::protocol::{self, DaemonRequest, DaemonResponse};
use crate::errors::AppError;

/// Dedicated daemon connection for versioned session-activity updates.
///
/// Uses long-poll requests (`wait_session_updates`) so the app can stay
/// event-driven above the daemon while scanner polling remains daemon-owned.
pub struct DaemonSessionListener {
    stream: TcpStream,
    reader: BufReader<TcpStream>,
    next_id: u64,
    /// The WSL distro the daemon runs in, if any — the distro whose token file
    /// this connection has to authenticate with.
    wsl_distro: Option<String>,
    auth_token: Option<String>,
}

impl DaemonSessionListener {
    /// Connect for long-poll session updates, authenticating against `wsl_distro`.
    ///
    /// The token has to come from the distro the daemon was started in: on
    /// Windows `read_auth_token()` reads whichever distro is currently default,
    /// so a daemon in any other distro rejects the connection and the focus
    /// bridge — the app's only live tmux-focus transport — goes silent.
    ///
    /// The token is read once here, which is also the refresh: a rejected poll
    /// returns an error, the bridge drops the listener and connects again, and
    /// this reads the token file anew.
    pub fn connect(addr: &str, wsl_distro: Option<&str>) -> Result<Self, AppError> {
        let stream = TcpStream::connect(addr).map_err(|e| {
            AppError::Io(std::io::Error::new(
                e.kind(),
                format!("Session listener connect to {addr} failed: {e}"),
            ))
        })?;
        stream.set_nodelay(true).map_err(|e| {
            AppError::Io(std::io::Error::new(
                e.kind(),
                format!("Session listener TCP_NODELAY setup failed for {addr}: {e}"),
            ))
        })?;
        let reader = BufReader::new(stream.try_clone().map_err(AppError::Io)?);
        let auth_token = crate::daemon::auth::read_auth_token_for_distro(wsl_distro);

        Ok(Self {
            stream,
            reader,
            next_id: 1,
            wsl_distro: wsl_distro.map(ToOwned::to_owned),
            auth_token,
        })
    }

    /// The distro this connection authenticated against.
    pub fn auth_distro(&self) -> Option<&str> {
        self.wsl_distro.as_deref()
    }

    /// The protocol version of the daemon on the other end of *this* socket.
    ///
    /// The bridge gates on the connection it is about to consume rather than on
    /// the shared provider's connected flag: a daemon replaced under a running
    /// app leaves that flag true until the health monitor next pings, and the
    /// bridge would meanwhile seed focus from whatever build now holds the port.
    pub fn ping_protocol_version(&mut self) -> Result<u32, AppError> {
        const PING_TIMEOUT: Duration = Duration::from_secs(5);

        let id = format!("sp{}", self.next_id);
        self.next_id += 1;

        let request = DaemonRequest::ping(&id).with_auth(self.auth_token.clone());
        let result = self.call(&request, PING_TIMEOUT, protocol::method::PING)?;
        let ping: protocol::PingResult = serde_json::from_value(result)
            .map_err(|e| AppError::InvalidPath(format!("Deserialize ping result failed: {e}")))?;
        Ok(ping.protocol_version)
    }

    pub fn wait_for_updates(
        &mut self,
        since_version: u64,
        timeout: Duration,
    ) -> Result<protocol::WaitSessionUpdatesResult, AppError> {
        let id = format!("su{}", self.next_id);
        self.next_id += 1;

        let request = DaemonRequest::new(
            &id,
            protocol::method::WAIT_SESSION_UPDATES,
            protocol::WaitSessionUpdatesParams {
                since_version,
                timeout_ms: timeout.as_millis() as u64,
            },
        )
        .with_auth(self.auth_token.clone());

        // The daemon holds a long poll open for `timeout`; give the socket the
        // slack to carry the answer that arrives at the very end of it.
        let result = self.call(
            &request,
            timeout + Duration::from_secs(2),
            protocol::method::WAIT_SESSION_UPDATES,
        )?;
        serde_json::from_value(result).map_err(|e| {
            AppError::InvalidPath(format!(
                "Deserialize wait_session_updates result failed: {e}"
            ))
        })
    }

    /// Send one request on this connection and read the single line back.
    ///
    /// `label` is the method name the error messages name, so a failed ping and
    /// a failed long poll stay distinguishable in the log.
    fn call(
        &mut self,
        request: &DaemonRequest,
        read_timeout: Duration,
        label: &str,
    ) -> Result<serde_json::Value, AppError> {
        let json = serde_json::to_string(request)
            .map_err(|e| AppError::InvalidPath(format!("Serialize {label} request failed: {e}")))?;
        self.stream
            .write_all(json.as_bytes())
            .map_err(AppError::Io)?;
        self.stream.write_all(b"\n").map_err(AppError::Io)?;
        self.stream.flush().map_err(AppError::Io)?;

        self.stream
            .set_read_timeout(Some(read_timeout))
            .map_err(AppError::Io)?;

        let mut line = String::new();
        self.reader.read_line(&mut line).map_err(AppError::Io)?;
        if line.trim().is_empty() {
            return Err(AppError::InvalidPath(format!(
                "Daemon returned empty {label} response"
            )));
        }

        let response: DaemonResponse = serde_json::from_str(&line)
            .map_err(|e| AppError::InvalidPath(format!("Parse {label} response failed: {e}")))?;

        if let Some(err) = response.error {
            return Err(AppError::InvalidPath(format!(
                "Daemon {label} error [{}]: {}",
                err.code, err.message
            )));
        }

        Ok(response.result.unwrap_or(serde_json::Value::Null))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    // Regression: 07ab6c5 deleted the hook -> file -> inotify focus chain, making
    // this long-poll listener the app's only live tmux-focus transport. It read
    // its auth token with `read_auth_token()` — `read_auth_token_for_distro(None)`
    // — which on Windows probes whichever WSL distro is currently default, not
    // the one the daemon was started in. A daemon in a non-default distro
    // rejected the token and no focus update ever arrived.
    #[test]
    fn the_listener_reads_its_token_from_the_daemons_own_distro() {
        let server = TcpListener::bind("127.0.0.1:0").expect("bind test listener");
        let addr = server.local_addr().expect("test listener addr");
        let accept_thread = std::thread::spawn(move || {
            let (_stream, _) = server.accept().expect("accept session listener");
            std::thread::sleep(Duration::from_millis(100));
        });

        let listener = DaemonSessionListener::connect(&addr.to_string(), Some("Taurhaus-Ubuntu"))
            .expect("connect session listener");

        assert_eq!(listener.auth_distro(), Some("Taurhaus-Ubuntu"));
        assert_eq!(
            listener.auth_token,
            crate::daemon::auth::read_auth_token_for_distro(Some("Taurhaus-Ubuntu")),
            "the listener must present the token of the distro the daemon runs in"
        );

        drop(listener);
        accept_thread.join().expect("join accept thread");
    }

    #[test]
    fn session_listener_connect_enables_tcp_nodelay() {
        let server = TcpListener::bind("127.0.0.1:0").expect("bind test listener");
        let addr = server.local_addr().expect("test listener addr");
        let accept_thread = std::thread::spawn(move || {
            let (_stream, _) = server.accept().expect("accept session listener");
            std::thread::sleep(Duration::from_millis(100));
        });

        let listener = DaemonSessionListener::connect(&addr.to_string(), None)
            .expect("connect session listener");
        assert!(listener.stream.nodelay().unwrap());

        drop(listener);
        accept_thread.join().expect("join accept thread");
    }
}
