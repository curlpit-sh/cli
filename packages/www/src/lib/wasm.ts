import initWasm, {
  import_curl as wasmImportCurl,
  process_request as wasmProcessRequest,
  render_export as wasmRenderExport,
} from "../../curlpit-wasm/pkg/curlpit_wasm.js";
import wasmModule from "../../curlpit-wasm/pkg/curlpit_wasm_bg.wasm";

export interface WasmHeader {
  name: string;
  value: string;
}

export interface WasmRequest {
  method: string;
  url: string;
  headers: WasmHeader[];
  body?: string | null;
}

export interface WasmInterpolationDetail {
  key: string;
  value: string;
}

export interface WasmProcessedRequest {
  request: WasmRequest;
  interpolation: WasmInterpolationDetail[];
}

export interface WasmImportResult {
  contents: string;
  suggested_filename?: string | null;
  method: string;
  url: string;
  warnings: string[];
}

let wasmReady: Promise<void> | null = null;

export function ensureWasmReady(): Promise<void> {
  if (!wasmReady) {
    wasmReady = initWasm({ module_or_path: wasmModule }).then(() => undefined);
  }
  return wasmReady;
}

export async function processRequest(
  curl: string,
  env: Record<string, string>,
): Promise<WasmProcessedRequest> {
  await ensureWasmReady();
  return wasmProcessRequest(curl, env) as WasmProcessedRequest;
}

export async function importCurl(
  command: string,
  templateVars: Record<string, string>,
  envVars: Record<string, string>,
): Promise<WasmImportResult> {
  await ensureWasmReady();
  return wasmImportCurl(command, templateVars, envVars) as WasmImportResult;
}

export async function renderExport(
  name: string,
  curl: string,
  env: Record<string, string>,
): Promise<string> {
  await ensureWasmReady();
  return wasmRenderExport(name, curl, env) as string;
}
