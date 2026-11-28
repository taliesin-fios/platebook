// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

//! A tiny plain-text SMTP server on 127.0.0.1 for transport tests: it can
//! accept mail, reject the recipient (permanently or transiently), or accept
//! the connection and never answer (a timeout). Nothing leaves the machine.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use chef_server::config::{Config, EmailTls};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;

#[derive(Clone, Copy, Debug)]
pub enum Mode {
    /// 250 to everything; messages are kept.
    Accept,
    /// `550 5.1.1` at RCPT TO.
    RejectRecipient,
    /// `451 4.3.0` at RCPT TO for the first `n` connections, then accept.
    TransientThenAccept(usize),
    /// Accept the TCP connection and never send a greeting.
    Silent,
    /// `535 5.7.8` at MAIL FROM: the server refuses our login.
    AuthFailed,
}

#[derive(Clone)]
pub struct FakeSmtp {
    pub port: u16,
    /// Raw DATA of each accepted message.
    pub messages: Arc<Mutex<Vec<String>>>,
    pub connections: Arc<AtomicUsize>,
    mode: Arc<Mutex<Mode>>,
}

impl FakeSmtp {
    pub async fn start(mode: Mode) -> FakeSmtp {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let fake = FakeSmtp {
            port: listener.local_addr().unwrap().port(),
            messages: Arc::default(),
            connections: Arc::default(),
            mode: Arc::new(Mutex::new(mode)),
        };
        let f = fake.clone();
        tokio::spawn(async move {
            while let Ok((sock, _)) = listener.accept().await {
                let n = f.connections.fetch_add(1, Ordering::SeqCst);
                let f = f.clone();
                tokio::spawn(async move { f.serve(sock, n).await });
            }
        });
        fake
    }

    pub fn set_mode(&self, mode: Mode) {
        *self.mode.lock().unwrap() = mode;
    }

    pub fn messages(&self) -> Vec<String> {
        self.messages.lock().unwrap().clone()
    }

    pub fn connections(&self) -> usize {
        self.connections.load(Ordering::SeqCst)
    }

    /// `base` pointed at this server: plain SMTP, no login.
    pub fn config(&self, base: Config) -> Config {
        Config {
            email_host: "127.0.0.1".into(),
            email_port: self.port,
            email_user: String::new(),
            email_pass: String::new(),
            email_tls: EmailTls::None,
            email_capture: false,
            email_from: "Platebook <noreply@example.com>".into(),
            ..base
        }
    }

    async fn serve(&self, sock: tokio::net::TcpStream, n: usize) {
        let mode = *self.mode.lock().unwrap();
        if let Mode::Silent = mode {
            tokio::time::sleep(std::time::Duration::from_secs(120)).await;
            drop(sock);
            return;
        }
        let (r, mut w) = sock.into_split();
        let mut lines = BufReader::new(r).lines();
        let _ = w.write_all(b"220 fake.test ESMTP\r\n").await;
        while let Ok(Some(line)) = lines.next_line().await {
            let cmd = line.to_ascii_uppercase();
            let reply: &[u8] = if cmd.starts_with("EHLO") || cmd.starts_with("HELO") {
                b"250-fake.test\r\n250 8BITMIME\r\n"
            } else if cmd.starts_with("MAIL FROM") && matches!(mode, Mode::AuthFailed) {
                b"535 5.7.8 Authentication credentials invalid\r\n"
            } else if cmd.starts_with("MAIL FROM")
                || cmd.starts_with("RSET")
                || cmd.starts_with("NOOP")
            {
                b"250 OK\r\n"
            } else if cmd.starts_with("RCPT TO") {
                match mode {
                    Mode::RejectRecipient => b"550 5.1.1 No such user here\r\n",
                    Mode::TransientThenAccept(k) if n < k => b"451 4.3.0 Try again later\r\n",
                    _ => b"250 OK\r\n",
                }
            } else if cmd.starts_with("DATA") {
                let _ = w
                    .write_all(b"354 End data with <CR><LF>.<CR><LF>\r\n")
                    .await;
                let mut data = String::new();
                while let Ok(Some(l)) = lines.next_line().await {
                    if l == "." {
                        break;
                    }
                    data.push_str(&l);
                    data.push('\n');
                }
                self.messages.lock().unwrap().push(data);
                b"250 OK queued\r\n"
            } else if cmd.starts_with("QUIT") {
                let _ = w.write_all(b"221 Bye\r\n").await;
                return;
            } else {
                b"502 Command not implemented\r\n"
            };
            if w.write_all(reply).await.is_err() {
                return;
            }
        }
    }
}
