use std::fmt;

use keyring::{Entry, Error as KeyringError};

pub const KEYRING_SERVICE: &str = "com.doustack.dscode";
pub const KEYRING_ACCOUNT: &str = "doustack-api-key";

pub trait CredentialStore {
    fn load(&self) -> Result<Option<String>, CredentialError>;
    fn save(&self, credential: &str) -> Result<(), CredentialError>;
    fn delete(&self) -> Result<bool, CredentialError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CredentialError {
    EmptyCredential,
    Backend {
        operation: &'static str,
        message: String,
    },
}

impl fmt::Display for CredentialError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyCredential => f.write_str("credential cannot be empty"),
            Self::Backend { operation, message } => {
                write!(f, "system credential store {operation} failed: {message}")
            }
        }
    }
}

impl std::error::Error for CredentialError {}

#[derive(Debug, Default, Clone, Copy)]
pub struct SystemCredentialStore;

impl SystemCredentialStore {
    fn entry() -> Result<Entry, CredentialError> {
        Entry::new(KEYRING_SERVICE, KEYRING_ACCOUNT)
            .map_err(|error| backend_error("initialization", error))
    }
}

impl CredentialStore for SystemCredentialStore {
    fn load(&self) -> Result<Option<String>, CredentialError> {
        match Self::entry()?.get_password() {
            Ok(credential) => Ok(Some(credential)),
            Err(KeyringError::NoEntry) => Ok(None),
            Err(error) => Err(backend_error("read", error)),
        }
    }

    fn save(&self, credential: &str) -> Result<(), CredentialError> {
        if credential.trim().is_empty() {
            return Err(CredentialError::EmptyCredential);
        }
        Self::entry()?
            .set_password(credential)
            .map_err(|error| backend_error("write", error))
    }

    fn delete(&self) -> Result<bool, CredentialError> {
        match Self::entry()?.delete_credential() {
            Ok(()) => Ok(true),
            Err(KeyringError::NoEntry) => Ok(false),
            Err(error) => Err(backend_error("delete", error)),
        }
    }
}

fn backend_error(operation: &'static str, error: KeyringError) -> CredentialError {
    CredentialError::Backend {
        operation,
        message: error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_credentials_are_rejected_before_keyring_access() {
        let error = SystemCredentialStore
            .save("  ")
            .expect_err("reject empty credential");

        assert_eq!(error, CredentialError::EmptyCredential);
    }
}
