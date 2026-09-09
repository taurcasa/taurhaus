//! Minimal RFC 6455 client for the pinned local Unix endpoint. No extensions.
use super::HostOperationLock;
use base64::{engine::general_purpose::STANDARD, Engine};
use rand::RngCore;
use std::io::{BufReader, Read, Write};
use std::os::unix::net::UnixStream;

pub(super) const FRAME_LIMIT: usize = 65_536;
pub(super) struct WebSocket(BufReader<UnixStream>);
impl WebSocket {
    pub fn connect(stream: UnixStream, guard: &HostOperationLock) -> Result<Self, String> {
        let mut socket = Self(BufReader::with_capacity(1024, stream));
        let mut nonce = [0; 16];
        rand::rngs::OsRng.fill_bytes(&mut nonce);
        let key = STANDARD.encode(nonce);
        socket.write_all(format!("GET / HTTP/1.1\r\nHost: localhost\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: {key}\r\nSec-WebSocket-Version: 13\r\n\r\n").as_bytes(), guard)?;
        let mut headers = Vec::new();
        while !headers.ends_with(b"\r\n\r\n") {
            if headers.len() == 8192 {
                return Err("host Upgrade headers exceed 8 KiB".into());
            }
            let mut byte = [0];
            socket.read_exact(&mut byte, guard)?;
            headers.push(byte[0]);
        }
        let headers = std::str::from_utf8(&headers).map_err(|_| "invalid Upgrade headers")?;
        let mut lines = headers.split("\r\n");
        let status = lines.next().unwrap_or_default();
        if status.split_whitespace().take(2).collect::<Vec<_>>() != ["HTTP/1.1", "101"] {
            return Err("host refused WebSocket Upgrade".into());
        }
        let mut values = std::collections::HashMap::new();
        for line in lines.filter(|line| !line.is_empty()) {
            let (name, value) = line.split_once(':').ok_or("invalid Upgrade header")?;
            if values
                .insert(name.to_ascii_lowercase(), value.trim())
                .is_some()
            {
                return Err("duplicate Upgrade header".into());
            }
        }
        let accept = STANDARD.encode(sha1(
            format!("{key}258EAFA5-E914-47DA-95CA-C5AB0DC85B11").as_bytes(),
        ));
        if !values
            .get("upgrade")
            .is_some_and(|v| v.eq_ignore_ascii_case("websocket"))
            || !values.get("connection").is_some_and(|v| {
                v.split(',')
                    .any(|v| v.trim().eq_ignore_ascii_case("upgrade"))
            })
            || values.get("sec-websocket-accept") != Some(&accept.as_str())
            || values.contains_key("sec-websocket-extensions")
            || values.contains_key("sec-websocket-protocol")
        {
            return Err("invalid WebSocket Upgrade accept".into());
        }
        Ok(socket)
    }

    fn read_exact(
        &mut self,
        mut bytes: &mut [u8],
        guard: &HostOperationLock,
    ) -> Result<(), String> {
        while !bytes.is_empty() {
            // Buffered Upgrade bytes (including a following frame) need no
            // syscall. Refresh the total deadline only before a socket read.
            if self.0.buffer().is_empty() {
                self.0
                    .get_ref()
                    .set_read_timeout(Some(guard.remaining().map_err(|e| e.to_string())?))
                    .map_err(|e| e.to_string())?;
            }
            let n = self
                .0
                .read(bytes)
                .map_err(|_| "host read failed; outcome may be unknown")?;
            if n == 0 {
                return Err("host connection closed".into());
            }
            bytes = &mut bytes[n..];
        }
        Ok(())
    }
    fn write_all(&mut self, mut bytes: &[u8], guard: &HostOperationLock) -> Result<(), String> {
        while !bytes.is_empty() {
            self.0
                .get_ref()
                .set_write_timeout(Some(guard.remaining().map_err(|e| e.to_string())?))
                .map_err(|e| e.to_string())?;
            let n = self
                .0
                .get_mut()
                .write(bytes)
                .map_err(|_| "host write failed; outcome may be unknown")?;
            if n == 0 {
                return Err("host connection closed during write".into());
            }
            bytes = &bytes[n..];
        }
        Ok(())
    }
    pub fn send(
        &mut self,
        opcode: u8,
        payload: &[u8],
        guard: &HostOperationLock,
    ) -> Result<(), String> {
        if payload.len() > FRAME_LIMIT {
            return Err("host frame exceeds 64 KiB".into());
        }
        let mut frame = vec![0x80 | opcode];
        match payload.len() {
            n @ 0..=125 => frame.push(0x80 | n as u8),
            n @ 126..=65535 => {
                frame.push(0xfe);
                frame.extend_from_slice(&(n as u16).to_be_bytes());
            }
            n => {
                frame.push(0xff);
                frame.extend_from_slice(&(n as u64).to_be_bytes());
            }
        }
        let mut mask = [0; 4];
        rand::rngs::OsRng.fill_bytes(&mut mask);
        frame.extend_from_slice(&mask);
        frame.extend(
            payload
                .iter()
                .enumerate()
                .map(|(i, byte)| byte ^ mask[i % 4]),
        );
        self.write_all(&frame, guard)
    }
    pub fn read(&mut self, guard: &HostOperationLock) -> Result<Vec<u8>, String> {
        let mut message = Vec::new();
        let mut fragmented = false;
        // Control/continuation frames cannot evade the bounded event budget.
        for _ in 0..128 {
            let mut header = [0; 2];
            self.read_exact(&mut header, guard)?;
            let fin = header[0] & 0x80 != 0;
            let opcode = header[0] & 15;
            if header[0] & 0x70 != 0 || header[1] & 0x80 != 0 {
                return Err("unsupported or masked server frame".into());
            }
            let size = header[1] & 127;
            let length = match size {
                126 => {
                    let mut b = [0; 2];
                    self.read_exact(&mut b, guard)?;
                    u16::from_be_bytes(b) as u64
                }
                127 => {
                    let mut b = [0; 8];
                    self.read_exact(&mut b, guard)?;
                    u64::from_be_bytes(b)
                }
                n => n as u64,
            };
            if length > FRAME_LIMIT as u64 {
                return Err("host frame exceeds 64 KiB".into());
            }
            if (size == 126 && length < 126)
                || (size == 127 && length <= 65535)
                || (opcode >= 8 && (!fin || length > 125))
            {
                return Err("invalid WebSocket frame length".into());
            }
            let mut payload = vec![0; length as usize];
            self.read_exact(&mut payload, guard)?;
            match opcode {
                8 => {
                    if payload.len() == 1 {
                        return Err("invalid close frame".into());
                    }
                    if payload.len() >= 2 {
                        let code = u16::from_be_bytes([payload[0], payload[1]]);
                        if !(matches!(code, 1000..=1003 | 1007..=1014 | 3000..=4999))
                            || std::str::from_utf8(&payload[2..]).is_err()
                        {
                            return Err("invalid close frame".into());
                        }
                    }
                    let _ = self.send(8, &payload, guard);
                    return Err("host WebSocket closed".into());
                }
                9 => {
                    self.send(10, &payload, guard)?;
                    continue;
                }
                10 => continue,
                1 if !fragmented => fragmented = true,
                0 if fragmented => (),
                _ => return Err("unexpected WebSocket opcode/continuation".into()),
            }
            if message.len() + payload.len() > FRAME_LIMIT {
                return Err("host message exceeds 64 KiB".into());
            }
            message.extend(payload);
            if fin {
                std::str::from_utf8(&message).map_err(|_| "invalid WebSocket text")?;
                return Ok(message);
            }
        }
        Err("host WebSocket frame limit reached".into())
    }
}

// SHA-1 is required only for RFC 6455's public handshake checksum, not security.
fn sha1(input: &[u8]) -> [u8; 20] {
    let mut padded = input.to_vec();
    padded.push(0x80);
    while padded.len() % 64 != 56 {
        padded.push(0);
    }
    padded.extend_from_slice(&(input.len() as u64 * 8).to_be_bytes());
    let mut state = [
        0x67452301u32,
        0xefcdab89,
        0x98badcfe,
        0x10325476,
        0xc3d2e1f0,
    ];
    let (blocks, _) = padded.as_chunks::<64>();
    for chunk in blocks {
        let mut words = [0u32; 80];
        let (block_words, _) = chunk.as_chunks::<4>();
        for (i, bytes) in block_words.iter().enumerate() {
            words[i] = u32::from_be_bytes(*bytes);
        }
        for i in 16..80 {
            words[i] = (words[i - 3] ^ words[i - 8] ^ words[i - 14] ^ words[i - 16]).rotate_left(1);
        }
        let [mut a, mut b, mut c, mut d, mut e] = state;
        for (i, word) in words.iter().enumerate() {
            let (f, k) = match i {
                0..=19 => ((b & c) | (!b & d), 0x5a827999),
                20..=39 => (b ^ c ^ d, 0x6ed9eba1),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8f1bbcdc),
                _ => (b ^ c ^ d, 0xca62c1d6),
            };
            let next = a
                .rotate_left(5)
                .wrapping_add(f)
                .wrapping_add(e)
                .wrapping_add(k)
                .wrapping_add(*word);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = next;
        }
        for (slot, value) in state.iter_mut().zip([a, b, c, d, e]) {
            *slot = slot.wrapping_add(value);
        }
    }
    let mut digest = [0; 20];
    let (digest_words, _) = digest.as_chunks_mut::<4>();
    for (bytes, value) in digest_words.iter_mut().zip(state) {
        *bytes = value.to_be_bytes();
    }
    digest
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn close_error_survives_failed_echo() {
        // Regression: c754d470 propagated a failed close echo as an ambiguous write.
        let tmp = tempfile::tempdir().unwrap();
        let guard =
            HostOperationLock::acquire(tmp.path(), "team", "seat", std::time::Duration::ZERO)
                .unwrap();
        let (client, mut server) = UnixStream::pair().unwrap();
        server.write_all(&[0x88, 2, 0x03, 0xe8]).unwrap();
        server.shutdown(std::net::Shutdown::Both).unwrap();
        let mut socket = WebSocket(BufReader::new(client));
        assert_eq!(socket.read(&guard).unwrap_err(), "host WebSocket closed");
    }
}
