use std::collections::HashMap;

use curlpit::web::{process_request as core_process_request, WebProcessError, WebProcessedRequest};
use serde_wasm_bindgen::{from_value, to_value};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn process_request(curl: &str, env: JsValue) -> Result<JsValue, JsValue> {
    let env_map: HashMap<String, String> = from_value(env)
        .map_err(|err| JsValue::from_str(&format!("Invalid env map: {err}")))?;

    convert_result(core_process_request(curl, &env_map))
}

fn convert_result(result: Result<WebProcessedRequest, WebProcessError>) -> Result<JsValue, JsValue> {
    match result {
        Ok(processed) => to_value(&processed)
            .map_err(|err| JsValue::from_str(&format!("Serialization error: {err}"))),
        Err(err) => Err(JsValue::from_str(&err.to_string())),
    }
}
