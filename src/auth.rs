//! VATSIM FSD token request used before the FSD protocol login.

use crate::Result;
use serde::Deserialize;
use std::io::Read;
use std::sync::Arc;
use std::time::Duration;

const AUTH_URL: &str = "https://auth.vatsim.net/api/fsd-jwt";

/// An FSD JWT returned after successful credential verification.
///
/// The token deliberately does not implement `Debug` to avoid accidental log output.
pub struct AuthToken(String);

impl AuthToken {
    /// Returns the token for the subsequent FSD login packet.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Deserialize)]
struct AuthReply {
    success: bool,
    token: Option<String>,
    error_msg: Option<String>,
}

/// Requests a VATSIM FSD JWT without writing the password to application logs.
pub struct AuthClient {
    endpoint: String,
    timeout: Duration,
}

/// Stores a VATSIM password in the operating system's credential vault.
#[derive(Debug, Default, Clone, Copy)]
pub struct PasswordStore;

impl PasswordStore {
    const SERVICE: &'static str = "ovPilot VATSIM";

    /// Loads the saved password for one VATSIM CID, if present.
    pub fn load(&self, cid: &str) -> Result<Option<String>> {
        let entry = self.entry(cid)?;
        match entry.get_password() {
            Ok(password) => Ok(Some(password)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    /// Saves a password only in the system credential vault.
    pub fn save(&self, cid: &str, password: &str) -> Result<()> {
        if password.is_empty() {
            return Err("VATSIM password is required".into());
        }
        self.entry(cid)?.set_password(password)?;
        Ok(())
    }

    /// Removes the saved password for one VATSIM CID.
    pub fn delete(&self, cid: &str) -> Result<()> {
        match self.entry(cid)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(error.into()),
        }
    }

    fn entry(&self, cid: &str) -> Result<keyring::Entry> {
        if cid.is_empty() || !cid.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err("VATSIM CID must contain only digits".into());
        }
        Ok(keyring::Entry::new(Self::SERVICE, cid)?)
    }
}

impl Default for AuthClient {
    fn default() -> Self {
        Self::new()
    }
}

impl AuthClient {
    /// Creates a client for the official authentication endpoint.
    pub fn new() -> Self {
        Self {
            endpoint: AUTH_URL.into(),
            timeout: Duration::from_secs(15),
        }
    }

    /// Requests a short-lived token for a numeric VATSIM CID and password.
    pub fn authenticate(&self, cid: &str, password: &str) -> Result<AuthToken> {
        if cid.is_empty() || !cid.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err("VATSIM CID must contain only digits".into());
        }
        if password.is_empty() {
            return Err("VATSIM password is required".into());
        }
        let body = serde_json::json!({ "cid": cid, "password": password }).to_string();
        let tls = ureq::native_tls::TlsConnector::new()?;
        let response = ureq::AgentBuilder::new()
            .timeout(self.timeout)
            .tls_connector(Arc::new(tls))
            .build()
            .post(&self.endpoint)
            .set("Content-Type", "application/json")
            .set("Accept", "application/json")
            .send_string(&body);
        match response {
            Ok(response) | Err(ureq::Error::Status(_, response)) => {
                let reply: AuthReply =
                    serde_json::from_reader(response.into_reader().take(64 * 1024))?;
                parse_reply(reply)
            }
            Err(error) => Err(format!("VATSIM authentication request failed: {error}").into()),
        }
    }

    #[cfg(test)]
    fn at(endpoint: String) -> Self {
        Self {
            endpoint,
            timeout: Duration::from_secs(2),
        }
    }
}

fn parse_reply(reply: AuthReply) -> Result<AuthToken> {
    if reply.success {
        match reply.token.filter(|token| !token.is_empty()) {
            Some(token) => Ok(AuthToken(token)),
            None => Err("authentication succeeded without a token".into()),
        }
    } else {
        Err(reply
            .error_msg
            .unwrap_or_else(|| "authentication failed".into())
            .into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    #[test]
    #[ignore = "writes a temporary credential to the operating system vault"]
    fn saves_and_removes_a_temporary_password() {
        let cid = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
            .to_string();
        let store = PasswordStore;
        store.save(&cid, "test-password").unwrap();
        assert_eq!(store.load(&cid).unwrap().as_deref(), Some("test-password"));
        store.delete(&cid).unwrap();
        assert!(store.load(&cid).unwrap().is_none());
    }

    #[test]
    fn accepts_token_and_reports_authentication_error() {
        let good: AuthReply =
            serde_json::from_str(r#"{"success":true,"token":"example"}"#).unwrap();
        assert_eq!(parse_reply(good).unwrap().as_str(), "example");
        let bad: AuthReply =
            serde_json::from_str(r#"{"success":false,"error_msg":"Invalid credentials"}"#).unwrap();
        assert!(parse_reply(bad).is_err());
    }

    #[test]
    fn posts_json_credentials_to_auth_endpoint() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let endpoint = format!("http://{}/api/fsd-jwt", listener.local_addr().unwrap());
        let worker = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut request = Vec::new();
            let mut buffer = [0u8; 1024];
            loop {
                let count = stream.read(&mut buffer).unwrap();
                request.extend_from_slice(&buffer[..count]);
                let text = String::from_utf8_lossy(&request);
                if let Some((head, body)) = text.split_once("\r\n\r\n") {
                    let length = head.lines().find_map(|line| {
                        line.to_ascii_lowercase()
                            .strip_prefix("content-length: ")
                            .and_then(|value| value.parse::<usize>().ok())
                    });
                    if body.len() >= length.unwrap_or(0) {
                        break;
                    }
                }
            }
            let request = String::from_utf8(request).unwrap();
            assert!(request.starts_with("POST /api/fsd-jwt HTTP/1.1"));
            assert!(request
                .to_ascii_lowercase()
                .contains("content-type: application/json"));
            let body: serde_json::Value =
                serde_json::from_str(request.split_once("\r\n\r\n").unwrap().1).unwrap();
            assert_eq!(
                body,
                serde_json::json!({"cid":"1234567","password":"secret"})
            );
            let response = "{\"success\":true,\"token\":\"test-token\"}";
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{response}",
                response.len()
            )
            .unwrap();
        });
        let token = AuthClient::at(endpoint)
            .authenticate("1234567", "secret")
            .unwrap();
        worker.join().unwrap();
        assert_eq!(token.as_str(), "test-token");
    }
}
