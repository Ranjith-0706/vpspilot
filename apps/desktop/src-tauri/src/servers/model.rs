use crate::error::AppError;
use serde::{Deserialize, Serialize};
use std::net::IpAddr;

pub const NAME_MAX: usize = 64;
pub const USERNAME_MAX: usize = 32;
pub const DESCRIPTION_MAX: usize = 500;
/// Windows Credential Manager blob limit (CRED_MAX_CREDENTIAL_BLOB_SIZE).
pub const SECRET_MAX_BYTES: usize = 2560;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthMethod {
    SshKey,
    Password,
}

impl AuthMethod {
    pub fn as_str(self) -> &'static str {
        match self {
            AuthMethod::SshKey => "ssh_key",
            AuthMethod::Password => "password",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "ssh_key" => Some(AuthMethod::SshKey),
            "password" => Some(AuthMethod::Password),
            _ => None,
        }
    }
}

/// Raw, untrusted input from the frontend.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerInput {
    pub name: String,
    pub host: String,
    pub port: i64,
    pub username: String,
    pub auth_method: AuthMethod,
    #[serde(default)]
    pub description: Option<String>,
}

/// Input that has passed validation and been normalized (trimmed).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedServer {
    pub name: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub auth_method: AuthMethod,
    pub description: Option<String>,
}

/// A stored server profile. Contains no secret material, only a credential reference.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerProfile {
    pub id: String,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub auth_method: AuthMethod,
    pub credential_ref: Option<String>,
    pub description: Option<String>,
    /// Unix epoch milliseconds.
    pub created_at: i64,
    pub updated_at: i64,
}

fn has_control_chars(s: &str) -> bool {
    s.chars().any(|c| c.is_control())
}

fn valid_hostname(host: &str) -> bool {
    if host.is_empty() || host.len() > 253 {
        return false;
    }
    let labels: Vec<&str> = host.split('.').collect();
    let labels_ok = labels.iter().all(|l| {
        !l.is_empty()
            && l.len() <= 63
            && !l.starts_with('-')
            && !l.ends_with('-')
            && l.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
    });
    // An all-numeric final label means something like "999.1.1.1": an invalid IP, not a hostname.
    let last_numeric = labels
        .last()
        .is_some_and(|l| !l.is_empty() && l.chars().all(|c| c.is_ascii_digit()));
    labels_ok && !last_numeric
}

pub fn validate_server_input(input: &ServerInput) -> Result<ValidatedServer, AppError> {
    let name = input.name.trim();
    if name.is_empty() {
        return Err(AppError::validation("name", "Server name is required."));
    }
    if name.chars().count() > NAME_MAX || has_control_chars(name) {
        return Err(AppError::validation(
            "name",
            format!(
                "Server name must be at most {NAME_MAX} characters with no control characters."
            ),
        ));
    }

    let host = input.host.trim();
    if host.is_empty() {
        return Err(AppError::validation(
            "host",
            "Hostname or IP address is required.",
        ));
    }
    if host.parse::<IpAddr>().is_err() && !valid_hostname(host) {
        return Err(AppError::validation(
            "host",
            "Enter a valid hostname or IP address.",
        ));
    }

    if !(1..=65535).contains(&input.port) {
        return Err(AppError::validation(
            "port",
            "SSH port must be between 1 and 65535.",
        ));
    }

    let username = input.username.trim();
    if username.is_empty() {
        return Err(AppError::validation(
            "username",
            "SSH username is required.",
        ));
    }
    let username_ok = username.chars().count() <= USERNAME_MAX
        && !username.starts_with('-')
        && username
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'));
    if !username_ok {
        return Err(AppError::validation(
            "username",
            format!(
                "Username must be at most {USERNAME_MAX} characters using letters, digits, '.', '_' or '-'."
            ),
        ));
    }

    let description = match input.description.as_deref().map(str::trim) {
        None | Some("") => None,
        Some(d) => {
            if d.chars().count() > DESCRIPTION_MAX {
                return Err(AppError::validation(
                    "description",
                    format!("Description must be at most {DESCRIPTION_MAX} characters."),
                ));
            }
            Some(d.to_string())
        }
    };

    Ok(ValidatedServer {
        name: name.to_string(),
        host: host.to_string(),
        port: input.port as u16,
        username: username.to_string(),
        auth_method: input.auth_method,
        description,
    })
}

pub fn validate_secret(secret: &str) -> Result<(), AppError> {
    if secret.trim().is_empty() {
        return Err(AppError::validation(
            "secret",
            "Credential must not be empty.",
        ));
    }
    if secret.len() > SECRET_MAX_BYTES {
        return Err(AppError::validation(
            "secret",
            format!(
                "Credential is too large for the OS credential store (max {SECRET_MAX_BYTES} bytes)."
            ),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input() -> ServerInput {
        ServerInput {
            name: "web-01".into(),
            host: "web-01.example".into(),
            port: 22,
            username: "deploy".into(),
            auth_method: AuthMethod::SshKey,
            description: None,
        }
    }

    fn field_of(r: Result<ValidatedServer, AppError>) -> Option<String> {
        r.err().and_then(|e| e.field)
    }

    #[test]
    fn accepts_valid_input_and_trims() {
        let mut i = input();
        i.name = "  web-01 ".into();
        i.description = Some("   ".into());
        let v = validate_server_input(&i).unwrap();
        assert_eq!(v.name, "web-01");
        assert_eq!(v.description, None);
    }

    #[test]
    fn accepts_ipv4_ipv6_and_hostnames() {
        for h in ["10.0.0.5", "2001:db8::1", "localhost", "a-b.example.com"] {
            let mut i = input();
            i.host = h.into();
            assert!(validate_server_input(&i).is_ok(), "{h}");
        }
    }

    #[test]
    fn rejects_bad_hosts() {
        for h in [
            "",
            " ",
            "bad host",
            "-a.com",
            "a..com",
            "999.1.1.1",
            "a_b.com",
            "[::1]",
            "x;rm -rf",
        ] {
            let mut i = input();
            i.host = h.into();
            assert_eq!(
                field_of(validate_server_input(&i)).as_deref(),
                Some("host"),
                "{h}"
            );
        }
    }

    #[test]
    fn rejects_bad_ports() {
        for p in [0, -1, 65536, 100_000] {
            let mut i = input();
            i.port = p;
            assert_eq!(field_of(validate_server_input(&i)).as_deref(), Some("port"));
        }
        for p in [1, 22, 65535] {
            let mut i = input();
            i.port = p;
            assert!(validate_server_input(&i).is_ok());
        }
    }

    #[test]
    fn rejects_bad_names_and_usernames() {
        let mut i = input();
        i.name = "  ".into();
        assert_eq!(field_of(validate_server_input(&i)).as_deref(), Some("name"));
        i.name = "x".repeat(NAME_MAX + 1);
        assert_eq!(field_of(validate_server_input(&i)).as_deref(), Some("name"));
        i.name = "line\nbreak".into();
        assert_eq!(field_of(validate_server_input(&i)).as_deref(), Some("name"));

        let mut i = input();
        for u in [
            "",
            "-oProxyCommand=x",
            "a b",
            "root;id",
            &"u".repeat(USERNAME_MAX + 1),
        ] {
            i.username = u.into();
            assert_eq!(
                field_of(validate_server_input(&i)).as_deref(),
                Some("username"),
                "{u}"
            );
        }
    }

    #[test]
    fn rejects_long_description() {
        let mut i = input();
        i.description = Some("d".repeat(DESCRIPTION_MAX + 1));
        assert_eq!(
            field_of(validate_server_input(&i)).as_deref(),
            Some("description")
        );
    }

    #[test]
    fn secret_validation() {
        assert!(validate_secret("hunter2").is_ok());
        assert!(validate_secret("   ").is_err());
        assert!(validate_secret(&"x".repeat(SECRET_MAX_BYTES + 1)).is_err());
    }

    #[test]
    fn auth_method_round_trip() {
        for m in [AuthMethod::SshKey, AuthMethod::Password] {
            assert_eq!(AuthMethod::parse(m.as_str()), Some(m));
        }
        assert_eq!(AuthMethod::parse("telnet"), None);
    }
}
