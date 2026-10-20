//! UniFFI surface for Kotlin (Android) and Swift (iOS).

use std::sync::{Arc, Mutex};

use crate::link::{Link, LinkAction, LinkConfig, LinkInput};
use crate::session::{Action, ConfigError, Input, Session, SessionConfig};

#[derive(Debug, uniffi::Error)]
pub enum CoreError {
    InvalidConfig { reason: String },
}

impl std::fmt::Display for CoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CoreError::InvalidConfig { reason } => write!(f, "invalid config: {reason}"),
        }
    }
}

impl From<ConfigError> for CoreError {
    fn from(e: ConfigError) -> Self {
        CoreError::InvalidConfig {
            reason: e.to_string(),
        }
    }
}

#[derive(uniffi::Object)]
pub struct CoreSession {
    inner: Mutex<Session>,
}

#[uniffi::export]
impl CoreSession {
    #[uniffi::constructor]
    pub fn new(config: SessionConfig) -> Result<Arc<Self>, CoreError> {
        Ok(Arc::new(Self {
            inner: Mutex::new(Session::new(config)?),
        }))
    }

    /// `now_ms` is a monotonic millisecond clock that keeps running in deep sleep.
    pub fn handle(&self, now_ms: u64, input: Input) -> Vec<Action> {
        match self.inner.lock() {
            Ok(mut s) => s.handle(now_ms, input),
            Err(_) => Vec::new(),
        }
    }
}

/// One monitoring relationship (ADR-017). Platforms talk only to this object.
#[derive(uniffi::Object)]
pub struct CoreLink {
    inner: Mutex<Link>,
}

#[uniffi::export]
impl CoreLink {
    #[uniffi::constructor]
    pub fn new(config: LinkConfig) -> Result<Arc<Self>, CoreError> {
        Ok(Arc::new(Self {
            inner: Mutex::new(Link::new(config)?),
        }))
    }

    /// `now_ms` is a monotonic millisecond clock that keeps running in deep sleep; call with
    /// `Tick` at least every 500 ms while monitoring.
    pub fn handle(&self, now_ms: u64, input: LinkInput) -> Vec<LinkAction> {
        match self.inner.lock() {
            Ok(mut l) => l.handle(now_ms, input),
            Err(_) => Vec::new(),
        }
    }

    /// The PSK-derived signaling room id.
    pub fn room_id(&self) -> String {
        match self.inner.lock() {
            Ok(l) => l.room_id().to_owned(),
            Err(_) => String::new(),
        }
    }
}

#[uniffi::export]
pub fn default_link_config(role: crate::protocol::Role, psk_b64url: String) -> LinkConfig {
    LinkConfig::with_defaults(role, psk_b64url)
}

/// Every status key, cause hint and security-warning key the core can emit.
#[uniffi::export]
pub fn message_keys() -> Vec<String> {
    crate::link::message_keys()
        .iter()
        .map(|k| (*k).to_owned())
        .collect()
}

#[uniffi::export]
pub fn derive_room_id(psk: Vec<u8>) -> String {
    crate::pairing::derive_room_id(&psk)
}

#[uniffi::export]
pub fn default_session_config(
    role: crate::protocol::Role,
    room_id: String,
    psk_b64url: Option<String>,
) -> SessionConfig {
    SessionConfig::with_defaults(role, room_id, psk_b64url)
}

#[uniffi::export]
pub fn encode_psk(bytes: Vec<u8>) -> String {
    crate::pairing::encode_psk(&bytes)
}

#[uniffi::export]
pub fn conformance_eval(case_json: String) -> String {
    crate::conformance::evaluate(&case_json)
}
