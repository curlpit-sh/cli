use std::collections::HashMap;

use curlpit::web::{
    import_curl_command_web as core_import_curl,
    process_request as core_process_request,
    render_export_template_web as core_render_export,
    WebProcessError, WebProcessedRequest,
};
use serde_wasm_bindgen::{from_value, to_value};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn process_request(curl: &str, env: JsValue) -> Result<JsValue, JsValue> {
    let env_map: HashMap<String, String> = from_value(env)
        .map_err(|err| JsValue::from_str(&format!("Invalid env map: {err}")))?;

    convert_result(core_process_request(curl, &env_map))
}

#[wasm_bindgen]
pub fn import_curl(
    command: &str,
    template_vars: JsValue,
    env_vars: JsValue,
) -> Result<JsValue, JsValue> {
    let template_map: HashMap<String, String> = from_value(template_vars)
        .map_err(|err| JsValue::from_str(&format!("Invalid template map: {err}")))?;
    let env_map: HashMap<String, String> = from_value(env_vars)
        .map_err(|err| JsValue::from_str(&format!("Invalid env map: {err}")))?;

    let result = core_import_curl(command, &template_map, &env_map)
        .map_err(|err| JsValue::from_str(&err.to_string()))?;
    to_value(&result).map_err(|err| JsValue::from_str(&format!("Serialization error: {err}")))
}

#[wasm_bindgen]
pub fn render_export(name: &str, curl: &str, env: JsValue) -> Result<String, JsValue> {
    let env_map: HashMap<String, String> = from_value(env)
        .map_err(|err| JsValue::from_str(&format!("Invalid env map: {err}")))?;

    core_render_export(name, curl, &env_map)
        .map_err(|err| JsValue::from_str(&err.to_string()))
}

fn convert_result(result: Result<WebProcessedRequest, WebProcessError>) -> Result<JsValue, JsValue> {
    match result {
        Ok(processed) => to_value(&processed)
            .map_err(|err| JsValue::from_str(&format!("Serialization error: {err}"))),
        Err(err) => Err(JsValue::from_str(&err.to_string())),
    }
}
