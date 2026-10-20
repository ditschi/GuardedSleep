//! wasm-bindgen surface for the web client and Node conformance tests. JSON strings keep the
//! boundary small; the shapes are the serde forms of `Input` / `Action`.

use wasm_bindgen::prelude::*;

use crate::link::{Link, LinkConfig, LinkInput};
use crate::session::{Input, Session, SessionConfig};

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
    serde_json::to_string(crate::link::message_keys()).unwrap_or_else(|_| "[]".into())
}

#[wasm_bindgen(js_name = deriveRoomId)]
pub fn derive_room_id(psk: &[u8]) -> String {
    crate::pairing::derive_room_id(psk)
}

#[wasm_bindgen(js_name = conformanceEval)]
pub fn conformance_eval(case_json: &str) -> String {
    crate::conformance::evaluate(case_json)
}
