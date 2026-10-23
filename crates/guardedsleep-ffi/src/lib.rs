//! UniFFI surface for Kotlin (Android) and Swift (iOS) over `guardedsleep-core` (ADR-015, ADR-020).
//! Wrapper objects only: all safety logic stays in the sans-IO core.

// The UniFFI macros expand to `unsafe` FFI glue. This is the one place besides the wasm facade
// where it is allowed; the workspace lint denies it everywhere else and the core forbids it.
#![allow(unsafe_code)]

use std::sync::{Arc, Mutex};

use guardedsleep_core::invite::{
    InviteError as CoreInviteError, build_invite as core_build_invite,
    parse_invite as core_parse_invite,
};
use guardedsleep_core::link::{Link, LinkAction, LinkConfig, LinkInput};
use guardedsleep_core::session::{Action, ConfigError, Input, Session, SessionConfig};

mod remote_types;

// The namespace keeps the Kotlin package `uniffi.guardedsleep_core` that the Android code imports.
uniffi::setup_scaffolding!("guardedsleep_core");

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
pub fn default_link_config(
    role: guardedsleep_core::protocol::Role,
    psk_b64url: String,
) -> LinkConfig {
    LinkConfig::with_defaults(role, psk_b64url)
}

/// Every status key, cause hint and security-warning key the core can emit.
#[uniffi::export]
pub fn message_keys() -> Vec<String> {
    guardedsleep_core::link::message_keys()
        .iter()
        .map(|k| (*k).to_owned())
        .collect()
}

#[uniffi::export]
pub fn derive_room_id(psk: Vec<u8>) -> String {
    guardedsleep_core::pairing::derive_room_id(&psk)
}

#[uniffi::export]
pub fn default_session_config(
    role: guardedsleep_core::protocol::Role,
    room_id: String,
    psk_b64url: Option<String>,
) -> SessionConfig {
    SessionConfig::with_defaults(role, room_id, psk_b64url)
}

#[uniffi::export]
pub fn encode_psk(bytes: Vec<u8>) -> String {
    guardedsleep_core::pairing::encode_psk(&bytes)
}

/// Why an invite text was rejected (`guardedsleep_core::invite::InviteError`). Kotlin sees it as
/// `InviteException`.
#[derive(Debug, uniffi::Error)]
pub enum InviteError {
    NotOurCode,
    IncompletePaste,
    Expired,
}

impl std::fmt::Display for InviteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            InviteError::NotOurCode => "not_our_code",
            InviteError::IncompletePaste => "incomplete_paste",
            InviteError::Expired => "expired",
        })
    }
}

impl From<CoreInviteError> for InviteError {
    fn from(e: CoreInviteError) -> Self {
        match e {
            CoreInviteError::NotOurCode => InviteError::NotOurCode,
            CoreInviteError::IncompletePaste => InviteError::IncompletePaste,
            CoreInviteError::Expired => InviteError::Expired,
        }
    }
}

/// A valid, unexpired invite. Holds the pairing key: no `Debug`; never log it or put it into an
/// Intent or Bundle.
#[derive(uniffi::Record)]
pub struct ParsedInvite {
    pub psk: Vec<u8>,
    pub urls: Vec<String>,
    pub expires_at_epoch_s: u64,
}

/// Parses a scanned or pasted v3 invite. `now_epoch_s` is the platform's wall clock in seconds.
#[uniffi::export]
pub fn parse_invite(text: String, now_epoch_s: u64) -> Result<ParsedInvite, InviteError> {
    let invite = core_parse_invite(&text, now_epoch_s)?;
    Ok(ParsedInvite {
        psk: invite.psk().to_vec(),
        urls: invite.urls.clone(),
        expires_at_epoch_s: invite.expires_at_epoch_s,
    })
}

/// Builds the v3 invite text for the QR code and the copy action.
#[uniffi::export]
pub fn build_invite(
    psk: Vec<u8>,
    urls: Vec<String>,
    expires_at_epoch_s: u64,
) -> Result<String, InviteError> {
    Ok(core_build_invite(&psk, &urls, expires_at_epoch_s)?)
}

#[uniffi::export]
pub fn conformance_eval(case_json: String) -> String {
    guardedsleep_core::conformance::evaluate(&case_json)
}
