/// Minimum password authentication for River.
//
// Single shared password, enabled only when `RIVER_PASSWORD` is set to a
// non-empty value. No users, roles, or ACLs. Session state lives in the
// TCP connection handler, never in the shared key-value store and never
// on disk.

/// Environment variable enabling authentication. Empty or unset = disabled.
pub const PASSWORD_ENV_VAR: &str = "RIVER_PASSWORD";

/// Server-side authentication configuration, built once at startup.
#[derive(Clone)]
pub struct AuthConfig {
    password: Option<String>,
}

impl AuthConfig {
    /// Authentication disabled: every connection starts authenticated and
    /// `AUTH` is rejected as unnecessary.
    pub fn disabled() -> Self {
        Self { password: None }
    }

    /// Fixed password (used by tests; production uses [`AuthConfig::from_env`]).
    pub fn with_password(password: impl Into<String>) -> Self {
        Self {
            password: Some(password.into()),
        }
    }

    /// Read `RIVER_PASSWORD` from the process environment.
    /// Empty or unset means disabled (never a default password).
    pub fn from_env() -> Self {
        match std::env::var(PASSWORD_ENV_VAR) {
            Ok(password) if !password.is_empty() => Self {
                password: Some(password),
            },
            _ => Self::disabled(),
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.password.is_some()
    }

    /// Constant-time comparison against the configured password.
    /// Returns false when authentication is disabled.
    pub fn verify(&self, supplied: &str) -> bool {
        match &self.password {
            Some(expected) => secure_eq(expected.as_bytes(), supplied.as_bytes()),
            None => false,
        }
    }
}

/// Debug never reveals the secret, so logging an `AuthConfig` is safe.
impl std::fmt::Debug for AuthConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AuthConfig")
            .field("enabled", &self.is_enabled())
            .finish()
    }
}

/// Constant-time equality: no early exit on content (length mismatch still
/// returns false immediately; only the length leaks, never the content).
fn secure_eq(expected: &[u8], supplied: &[u8]) -> bool {
    if expected.len() != supplied.len() {
        return false;
    }
    let mut diff = 0u8;
    for (a, b) in expected.iter().zip(supplied.iter()) {
        diff |= a ^ b;
    }
    diff == 0
}

#[cfg(test)]
mod tests {
    use super::{AuthConfig, secure_eq};

    #[test]
    fn equal_and_unequal_inputs() {
        assert!(secure_eq(b"secret", b"secret"));
        assert!(!secure_eq(b"secret", b"secreu"));
        assert!(!secure_eq(b"secret", b"short"));
        assert!(!secure_eq(b"secret", b"much longer input"));
        assert!(secure_eq(b"", b""));
    }

    #[test]
    fn disabled_by_default_without_env() {
        // Must not depend on the ambient environment.
        if std::env::var(super::PASSWORD_ENV_VAR).is_ok() {
            return;
        }
        let auth = AuthConfig::from_env();
        assert!(!auth.is_enabled());
        assert!(!auth.verify("anything"));
    }

    #[test]
    fn debug_redacts_the_secret() {
        let auth = AuthConfig {
            password: Some("s3cr3t".to_string()),
        };
        let rendered = format!("{auth:?}");
        assert!(rendered.contains("enabled"));
        assert!(!rendered.contains("s3cr3t"));
    }
}
