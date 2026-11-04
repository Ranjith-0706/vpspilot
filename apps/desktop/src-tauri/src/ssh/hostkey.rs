//! Host key identification and the trust decision.

use russh::keys::{HashAlg, PublicKeyOrCertificate};
use serde::Serialize;

/// A host key presented by a server during a connection attempt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ObservedKey {
    /// e.g. `ssh-ed25519`
    pub key_type: String,
    /// OpenSSH-style SHA-256 fingerprint, e.g. `SHA256:abc...`
    pub fingerprint: String,
}

/// A host key the user has explicitly trusted for a server profile.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HostKeyRecord {
    pub server_id: String,
    pub key_type: String,
    pub fingerprint: String,
    /// Unix epoch milliseconds.
    pub trusted_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostKeyDecision {
    /// Matches the stored key.
    Trusted,
    /// Nothing stored yet: the user must verify and trust it first.
    Unknown,
    /// Differs from the stored key: possible impersonation or a rebuilt server.
    Changed { expected: String },
}

pub fn evaluate(stored: Option<&HostKeyRecord>, observed: &ObservedKey) -> HostKeyDecision {
    match stored {
        None => HostKeyDecision::Unknown,
        Some(s) if s.key_type == observed.key_type && s.fingerprint == observed.fingerprint => {
            HostKeyDecision::Trusted
        }
        Some(s) => HostKeyDecision::Changed {
            expected: s.fingerprint.clone(),
        },
    }
}

/// Extracts the identity of a presented key. Host certificates are not supported (None).
pub fn observe(key: &PublicKeyOrCertificate) -> Option<ObservedKey> {
    match key {
        PublicKeyOrCertificate::PublicKey { key, .. } => Some(ObservedKey {
            key_type: key.algorithm().to_string(),
            fingerprint: key.fingerprint(HashAlg::Sha256).to_string(),
        }),
        PublicKeyOrCertificate::Certificate(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn observed(fp: &str) -> ObservedKey {
        ObservedKey {
            key_type: "ssh-ed25519".into(),
            fingerprint: fp.into(),
        }
    }

    fn stored(fp: &str, kt: &str) -> HostKeyRecord {
        HostKeyRecord {
            server_id: "s".into(),
            key_type: kt.into(),
            fingerprint: fp.into(),
            trusted_at: 0,
        }
    }

    #[test]
    fn unknown_when_nothing_stored() {
        assert_eq!(
            evaluate(None, &observed("SHA256:a")),
            HostKeyDecision::Unknown
        );
    }

    #[test]
    fn trusted_only_on_exact_match() {
        let s = stored("SHA256:a", "ssh-ed25519");
        assert_eq!(
            evaluate(Some(&s), &observed("SHA256:a")),
            HostKeyDecision::Trusted
        );
    }

    #[test]
    fn changed_on_different_fingerprint_or_type() {
        let s = stored("SHA256:a", "ssh-ed25519");
        assert_eq!(
            evaluate(Some(&s), &observed("SHA256:b")),
            HostKeyDecision::Changed {
                expected: "SHA256:a".into()
            }
        );
        let s = stored("SHA256:a", "ssh-rsa");
        assert!(matches!(
            evaluate(Some(&s), &observed("SHA256:a")),
            HostKeyDecision::Changed { .. }
        ));
    }
}
