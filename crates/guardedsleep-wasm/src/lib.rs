//! wasm-bindgen surface for the web client and Node conformance tests. JSON strings keep the
//! boundary small; the shapes are the serde forms of `Input` / `Action`. The TypeScript
//! definitions of those shapes are derived from the core types (tsify) and land in the generated
//! `.d.ts`.

// wasm-bindgen expands to `unsafe` glue. This is the one place besides the UniFFI facade where
// it is allowed; the workspace lint denies it everywhere else and the core forbids it.
#![allow(unsafe_code)]

use wasm_bindgen::prelude::*;

use guardedsleep_core::link::{Link, LinkConfig, LinkInput};
use guardedsleep_core::session::{Input, Session, SessionConfig};

#[wasm_bindgen]
pub struct WasmSession {
    inner: Session,
}

#[wasm_bindgen]
impl WasmSession {
    #[wasm_bindgen(constructor)]
    pub fn new(config_json: &str) -> Result<WasmSession, JsError> {
        let cfg: SessionConfig = serde_json::from_str(config_json)?;
        let inner = Session::new(cfg).map_err(|e| JsError::new(&e.to_string()))?;
        Ok(Self { inner })
    }

    /// Returns a JSON array of actions.
    pub fn handle(&mut self, now_ms: f64, input_json: &str) -> Result<String, JsError> {
        let input: Input = serde_json::from_str(input_json)?;
        let actions = self.inner.handle(now_ms.max(0.0) as u64, input);
        Ok(serde_json::to_string(&actions)?)
    }
}

/// ADR-017 link. `now_ms` must be monotonic (clamp `Date.now()` so it never goes backwards).
#[wasm_bindgen]
pub struct WasmLink {
    inner: Link,
}

#[wasm_bindgen]
impl WasmLink {
    #[wasm_bindgen(constructor)]
    pub fn new(config_json: &str) -> Result<WasmLink, JsError> {
        let cfg: LinkConfig = serde_json::from_str(config_json)?;
        let inner = Link::new(cfg).map_err(|e| JsError::new(&e.to_string()))?;
        Ok(Self { inner })
    }

    /// Returns a JSON array of `LinkAction`s.
    pub fn handle(&mut self, now_ms: f64, input_json: &str) -> Result<String, JsError> {
        let input: LinkInput = serde_json::from_str(input_json)?;
        let actions = self.inner.handle(now_ms.max(0.0) as u64, input);
        Ok(serde_json::to_string(&actions)?)
    }

    #[wasm_bindgen(js_name = roomId)]
    pub fn room_id(&self) -> String {
        self.inner.room_id().to_owned()
    }
}

/// JSON array of every UI key the core can emit.
#[wasm_bindgen(js_name = messageKeys)]
pub fn message_keys() -> String {
    serde_json::to_string(guardedsleep_core::link::message_keys()).unwrap_or_else(|_| "[]".into())
}

#[wasm_bindgen(js_name = deriveRoomId)]
pub fn derive_room_id(psk: &[u8]) -> String {
    guardedsleep_core::pairing::derive_room_id(psk)
}

/// Parses a scanned or pasted v3 invite. Returns JSON, `{"type":"ok","psk_b64url":..,"urls":[..],
/// "expires_at_epoch_s":..}` or `{"type":"error","error":"not_our_code"|"incomplete_paste"|"expired"}`;
/// the `packages/core-wasm` wrapper turns it into a typed result. `now_epoch_s` is wall-clock seconds.
#[wasm_bindgen(js_name = parseInvite)]
pub fn parse_invite(text: &str, now_epoch_s: f64) -> String {
    match guardedsleep_core::invite::parse_invite(text, now_epoch_s.max(0.0) as u64) {
        Ok(inv) => serde_json::json!({
            "type": "ok",
            "psk_b64url": guardedsleep_core::pairing::encode_psk(inv.psk()),
            "urls": inv.urls,
            "expires_at_epoch_s": inv.expires_at_epoch_s,
        }),
        Err(e) => serde_json::json!({ "type": "error", "error": e.as_str() }),
    }
    .to_string()
}

/// Builds the v3 invite text. Throws the snake_case error key if the inputs are not a valid invite.
#[wasm_bindgen(js_name = buildInvite)]
pub fn build_invite(
    psk: &[u8],
    urls: Vec<String>,
    expires_at_epoch_s: f64,
) -> Result<String, JsError> {
    guardedsleep_core::invite::build_invite(psk, &urls, expires_at_epoch_s.max(0.0) as u64)
        .map_err(|e| JsError::new(e.as_str()))
}

#[wasm_bindgen(js_name = conformanceEval)]
pub fn conformance_eval(case_json: &str) -> String {
    guardedsleep_core::conformance::evaluate(case_json)
}

// The test-only envelope entry points (fixed-nonce sealing, key export) must not exist in a build
// without the `conformance` feature, which is what ships.
#[cfg(all(test, not(feature = "conformance")))]
mod release_surface {
    #[test]
    fn envelope_test_entry_points_are_absent() {
        for kind in ["envelope_key", "envelope_seal", "envelope_open"] {
            let case = format!(r#"{{"kind":"{kind}"}}"#);
            let got = super::conformance_eval(&case);
            assert!(got.contains("unknown kind"), "{kind}: {got}");
        }
    }
}
