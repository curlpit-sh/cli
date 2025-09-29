import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import type { JSX } from "react";
import {
  Button,
  Dialog,
  Modal,
  ModalOverlay,
  Tab,
  TabList,
  TabPanel,
  Tabs,
  TextArea,
} from "react-aria-components";

import { playgroundExamples } from "../data/examples";
import {
  formatImportedVariables,
  parseEnvVariables,
  type EnvVars,
} from "../lib/env";
import {
  ensureWasmReady,
  importCurl,
  processRequest,
  renderExport,
  type WasmImportResult,
  type WasmProcessedRequest,
  type WasmRequest,
} from "../lib/wasm";

interface PlaygroundResponse {
  status: number;
  statusText: string;
  headers: Record<string, string>;
  body: string;
}

type RunResult =
  | { status: "idle" }
  | { status: "preparing"; timestamp: string; processed?: WasmProcessedRequest }
  | {
      status: "success";
      timestamp: string;
      processed: WasmProcessedRequest;
      response: PlaygroundResponse;
    }
  | {
      status: "error";
      timestamp: string;
      processed?: WasmProcessedRequest;
      error: string;
      fetchMessage?: string;
      fallbackRequest?: WasmRequest;
    };

interface ImportContext {
  command: string;
  templateVars: EnvVars;
  envVars: EnvVars;
}

const EXPORT_TEMPLATES = [{ value: "js-fetch", label: "JavaScript fetch()" }];

const initialExample = playgroundExamples[0];

function buildDefaultImportContext(): ImportContext {
  const apiBase = "https://httpbin.org";
  const traceId = `playground-${Math.random().toString(36).slice(2, 8)}`;
  const requestId = `req-${Date.now()}`;
  const timestamp = new Date().toISOString();

  const templateVars: EnvVars = {
    API_BASE: apiBase,
    TRACE_ID: traceId,
    USER_AGENT: "curlpit-playground/1.0",
    REQUEST_ID: requestId,
  };

  const envVars: EnvVars = {
    API_TOKEN: "demo-token-12345",
    API_SECRET: "secret-playground-67890",
  };

  const commandLines = [
    `curl -X POST ${apiBase}/anything/import-demo?trace=${traceId}`,
    `  -H "Authorization: Bearer ${envVars.API_TOKEN}"`,
    `  -H "User-Agent: ${templateVars.USER_AGENT}"`,
    `  -H "X-Playground-Trace: ${traceId}"`,
    `  -H "X-Api-Secret: ${envVars.API_SECRET}"`,
    `  -H "Accept: application/json"`,
    `  --json '{"message":"Imported from curlpit","request_id":"${requestId}","timestamp":"${timestamp}"}'`,
  ];

  const command = commandLines
    .map((line, index) => (index === commandLines.length - 1 ? line : `${line} \\`))
    .join("\n");

  return { command, templateVars, envVars };
}

function escapeHtml(text: string): string {
  return text.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
}

function syntaxHighlightJson(value: unknown): string {
  const json = escapeHtml(JSON.stringify(value, null, 2));
  return json
    .replace(/"([^"\\]+)":/g, "<span class=\"syntax-header-key\">\"$1\"</span>:")
    .replace(
      /: "([^"\\]*)"/g,
      ': <span class="syntax-header-value">"$1"</span>',
    )
    .replace(/: (\d+(?:\.\d+)?)/g, ': <span class="syntax-variable">$1</span>')
    .replace(
      /: (true|false|null)/g,
      ': <span class="syntax-method">$1</span>',
    );
}

function responseOk(status: number): boolean {
  return status >= 200 && status < 300;
}

function renderInterpolationDetails(details: WasmProcessedRequest["interpolation"]): JSX.Element {
  if (details.length === 0) {
    return <></>;
  }

  return (
    <div className="interpolation-info">
      {details.map((detail) => (
        <div key={detail.key} className="interpolation-line">
          <span className="variable-highlight">{`{${detail.key}}`}</span>
          <span className="arrow">→</span>
          <span className="syntax-header-value">{detail.value}</span>
        </div>
      ))}
    </div>
  );
}

export function Playground(): JSX.Element {
  const [curl, setCurl] = useState(initialExample.curl);
  const [variables, setVariables] = useState(initialExample.variables);
  const [exampleKey, setExampleKey] = useState(initialExample.value);
  const [selectedTab, setSelectedTab] = useState<string>("output");
  const [isRunning, setIsRunning] = useState(false);
  const [runResult, setRunResult] = useState<RunResult>({ status: "idle" });
  const [preview, setPreview] = useState<WasmProcessedRequest | null>(null);
  const [previewError, setPreviewError] = useState<string | null>(null);
  const [exportTemplate, setExportTemplate] = useState(EXPORT_TEMPLATES[0].value);
  const [exportSnippet, setExportSnippet] = useState("");
  const [exportStatus, setExportStatus] = useState<string | null>(null);
  const [copyState, setCopyState] = useState<"idle" | "copied">("idle");

  const [importContext, setImportContext] = useState<ImportContext>(() =>
    buildDefaultImportContext(),
  );
  const [importCommand, setImportCommand] = useState(importContext.command);
  const [importStatus, setImportStatus] = useState<string | null>(null);
  const [lastImportSummary, setLastImportSummary] = useState<string | null>(null);
  const [isImportOpen, setIsImportOpen] = useState(false);
  const importRef = useRef<HTMLTextAreaElement | null>(null);

  const envVars = useMemo(() => parseEnvVariables(variables), [variables]);

  useEffect(() => {
    ensureWasmReady().catch((error) => {
      console.error("Failed to initialise WASM", error);
    });
  }, []);

  useEffect(() => {
    let cancelled = false;

    async function updatePreview() {
      if (!curl.trim()) {
        setPreview(null);
        setPreviewError(null);
        return;
      }

      try {
        const processed = await processRequest(curl, envVars);
        if (!cancelled) {
          setPreview(processed);
          setPreviewError(null);
        }
      } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        if (!cancelled) {
          setPreview(null);
          setPreviewError(message);
        }
      }
    }

    void updatePreview();

    return () => {
      cancelled = true;
    };
  }, [curl, envVars]);

  useEffect(() => {
    let cancelled = false;

    async function generateExport() {
      if (!curl.trim()) {
        setExportSnippet("");
        setExportStatus("Add a curl template to render an export snippet.");
        return;
      }

      try {
        const snippet = await renderExport(exportTemplate, curl, envVars);
        if (!cancelled) {
          setExportSnippet(snippet);
          setExportStatus(`Generated template "${exportTemplate}" using current variables.`);
        }
      } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        if (!cancelled) {
          setExportSnippet("");
          setExportStatus(`Export failed: ${message}`);
        }
      }
    }

    void generateExport();

    return () => {
      cancelled = true;
    };
  }, [curl, envVars, exportTemplate]);

  useEffect(() => {
    if (!isImportOpen) {
      return;
    }

    const handle = window.setTimeout(() => {
      if (importRef.current) {
        importRef.current.focus();
        importRef.current.select();
      }
    }, 50);

    return () => window.clearTimeout(handle);
  }, [isImportOpen]);

  const resetImportContext = useCallback(() => {
    const context = buildDefaultImportContext();
    setImportContext(context);
    setImportCommand(context.command);
    setImportStatus(null);
  }, []);

  const loadExample = useCallback(
    (value: string) => {
      const example = playgroundExamples.find((item) => item.value === value);
      if (!example) {
        return;
      }

      setExampleKey(example.value);
      setCurl(example.curl);
      setVariables(example.variables);
    },
    [],
  );

  const clearVariables = useCallback(() => {
    setVariables("# Environment variables\n");
  }, []);

  const copyToClipboard = useCallback(async () => {
    if (!exportSnippet.trim() || !navigator.clipboard) {
      return;
    }

    try {
      await navigator.clipboard.writeText(exportSnippet);
      setCopyState("copied");
      window.setTimeout(() => setCopyState("idle"), 2000);
    } catch (error) {
      console.error("Failed to copy", error);
    }
  }, [exportSnippet]);

  const executeRequest = useCallback(async () => {
    if (isRunning) {
      return;
    }

    const timestamp = new Date().toISOString();
    setIsRunning(true);
    setSelectedTab("output");
    setRunResult({ status: "preparing", timestamp });

    try {
      const processed = await processRequest(curl, envVars);

      const request = processed.request;
      if (!request.url || !/^https?:\/\//i.test(request.url)) {
        throw new Error("Invalid URL. Must start with http:// or https://");
      }

      setRunResult({ status: "preparing", timestamp, processed });

      const headers = request.headers.reduce<Record<string, string>>((acc, header) => {
        acc[header.name] = header.value;
        return acc;
      }, {});

      const fetchInit: RequestInit = {
        method: request.method,
        headers,
      };

      if (
        request.body &&
        ["POST", "PUT", "PATCH", "DELETE"].includes(request.method.toUpperCase())
      ) {
        fetchInit.body = request.body;
      }

      const response = await fetch(request.url, fetchInit);
      const body = await response.text();
      const responsePayload: PlaygroundResponse = {
        status: response.status,
        statusText: response.statusText,
        headers: Object.fromEntries(response.headers.entries()),
        body,
      };

      setRunResult({
        status: "success",
        timestamp,
        processed,
        response: responsePayload,
      });
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      setRunResult((current) => ({
        status: "error",
        timestamp,
        processed: current.status === "preparing" ? current.processed : undefined,
        error: message,
        fetchMessage: message,
        fallbackRequest:
          current.status === "preparing" ? current.processed?.request : undefined,
      }));
    } finally {
      setIsRunning(false);
    }
  }, [curl, envVars, isRunning]);

  useEffect(() => {
    const handler = (event: KeyboardEvent) => {
      if ((event.metaKey || event.ctrlKey) && event.key === "Enter") {
        event.preventDefault();
        void executeRequest();
      }
    };

    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [executeRequest]);

  const handleImport = useCallback(async () => {
    const command = importCommand.trim();
    if (!command) {
      setImportStatus("Provide a curl command before importing.");
      return;
    }

    const templateVars: EnvVars = { ...importContext.templateVars };
    const envVarsForImport: EnvVars = { ...importContext.envVars };
    const currentVars = parseEnvVariables(variables);

    for (const [key, value] of Object.entries(currentVars)) {
      templateVars[key] = value;
      envVarsForImport[key] = value;
    }

    try {
      const result: WasmImportResult = await importCurl(
        command,
        templateVars,
        envVarsForImport,
      );

      setCurl(result.contents);
      setVariables(formatImportedVariables(templateVars, envVarsForImport));
      const summary = [
        `Imported ${result.method} ${result.url}`,
        result.suggested_filename
          ? `Suggested file: ${result.suggested_filename}`
          : null,
        result.warnings.length > 0
          ? `Warnings: ${result.warnings.join(" | ")}`
          : null,
      ]
        .filter(Boolean)
        .join("\n");

      setImportContext({ command, templateVars, envVars: envVarsForImport });
      setLastImportSummary(summary);
      setImportStatus(null);
      setIsImportOpen(false);
      setSelectedTab("output");
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      setImportStatus(`Import failed: ${message}`);
    }
  }, [importCommand, importContext, variables]);

  const renderOutputPanel = () => {
    if (runResult.status === "success") {
      const { timestamp, processed, response } = runResult;
      const request = processed.request;
      const statusClass = responseOk(response.status)
        ? "status-success"
        : "status-error";

      let bodyContent: JSX.Element | null = null;
      try {
        const json = JSON.parse(response.body);
        bodyContent = (
          <pre
            className="whitespace-pre-wrap"
            dangerouslySetInnerHTML={{ __html: syntaxHighlightJson(json) }}
          />
        );
      } catch {
        bodyContent = <pre className="whitespace-pre-wrap">{response.body}</pre>;
      }

      return (
        <div className="space-y-4">
          <div className="space-y-1">
            <div>
              <span className="dim">[{timestamp}]</span>
            </div>
            <div>
              <span className="syntax-method">{request.method}</span> {request.url}
            </div>
          </div>
          {processed.interpolation.length > 0 && (
            <div>
              <span className="dim">Variable Interpolation:</span>
              {renderInterpolationDetails(processed.interpolation)}
            </div>
          )}
          <div className={statusClass}>
            HTTP {response.status} {response.statusText}
          </div>
          {bodyContent}
        </div>
      );
    }

    if (runResult.status === "error") {
      return (
        <div className="space-y-3">
          <div className="status-error">Error: {runResult.error}</div>
          {runResult.fallbackRequest && (
            <div className="space-y-2">
              <div className="dim">
                Browser security (CORS) prevents direct API calls to most domains.
                Use the curlpit CLI or a CORS-enabled endpoint.
              </div>
              <div className="dim">--- Request Details ---</div>
              <div>
                <span className="syntax-method">
                  {runResult.fallbackRequest.method}
                </span>{" "}
                {runResult.fallbackRequest.url}
              </div>
              {runResult.fallbackRequest.headers.map((header) => (
                <div key={header.name}>
                  <span className="header-key">{header.name}:</span>{" "}
                  <span className="header-value">{header.value}</span>
                </div>
              ))}
              {runResult.fallbackRequest.body && (
                <pre className="whitespace-pre-wrap">
                  {runResult.fallbackRequest.body}
                </pre>
              )}
            </div>
          )}
        </div>
      );
    }

    if (preview) {
      return (
        <div className="space-y-3">
          <div className="dim">
            Ready to run. Press "Run" or Cmd/Ctrl+Enter to execute.
          </div>
          <div>
            <span className="dim">Request:</span>
            <div>
              <span className="syntax-method">{preview.request.method}</span>{" "}
              <span className="syntax-url">{preview.request.url}</span>
            </div>
          </div>
          {preview.interpolation.length > 0 && (
            <div>
              <span className="dim">Template variables will be interpolated:</span>
              {renderInterpolationDetails(preview.interpolation)}
            </div>
          )}
        </div>
      );
    }

    if (previewError) {
      return <div className="status-error">Preview failed: {previewError}</div>;
    }

    return (
      <div className="dim">
        Press "Run" or Cmd/Ctrl+Enter to execute the request after adding a curl
        template.
      </div>
    );
  };

  const renderHeadersPanel = () => {
    if (runResult.status !== "success") {
      return <div className="dim">Run a request to view response headers.</div>;
    }

    const entries = Object.entries(runResult.response.headers);
    if (entries.length === 0) {
      return <div className="dim">No response headers received.</div>;
    }

    return (
      <div className="space-y-2">
        <div className="dim">Response Headers:</div>
        <div className="space-y-1">
          {entries.map(([key, value]) => (
            <div key={key}>
              <span className="header-key">{key}:</span>{" "}
              <span className="header-value">{value}</span>
            </div>
          ))}
        </div>
      </div>
    );
  };

  const renderRawPanel = () => {
    if (runResult.status !== "success") {
      return <div className="dim">Run a request to inspect the raw body.</div>;
    }

    return (
      <div className="space-y-2">
        <div className="dim">Raw Response Body:</div>
        <pre className="whitespace-pre-wrap">{runResult.response.body}</pre>
      </div>
    );
  };

  return (
    <div className="page-shell">
      <ModalOverlay
        isOpen={isImportOpen}
        isDismissable
        onOpenChange={(open) => {
          setIsImportOpen(open);
          if (!open) {
            setImportStatus(null);
          }
        }}
        className="fixed inset-0 z-50 flex items-center justify-center bg-black/70 backdrop-blur-sm p-4"
      >
        <Modal className="w-full max-w-2xl">
          <Dialog
            className="space-y-5 rounded-lg border border-[var(--terminal-border)] bg-[var(--terminal-surface)] p-6 shadow-2xl focus:outline-none"
            aria-label="Import curl command"
          >
            <div className="space-y-2">
              <h2 className="text-lg font-semibold text-[var(--terminal-text)]">
                Import curl command
              </h2>
              <p className="import-hints">
                Use the defaults to see how curlpit promotes secrets to env vars and
                replaces stable values. Paste any curl command here to import it.
              </p>
            </div>
            <TextArea
              ref={importRef}
              value={importCommand}
              onChange={(event) => setImportCommand(event.target.value)}
              className="import-textarea"
              aria-label="Import curl command"
              spellCheck={false}
              autoFocus
            />
            <div className="grid gap-3 text-xs text-[var(--terminal-text)]">
              <div>
                <strong>Template placeholders</strong>
                <div className="grid gap-1 import-hints">
                  {Object.entries(importContext.templateVars)
                    .sort((a, b) => a[0].localeCompare(b[0]))
                    .map(([key, value]) => (
                      <div key={`template-${key}`}>
                        <code>{`{${key}}`}</code> ← <code>{value}</code>
                      </div>
                    ))}
                </div>
              </div>
              <div>
                <strong>Environment variables</strong>
                <div className="grid gap-1 import-hints">
                  {Object.entries(importContext.envVars)
                    .sort((a, b) => a[0].localeCompare(b[0]))
                    .map(([key, value]) => (
                      <div key={`env-${key}`}>
                        <code>{key}</code>=<code>{value}</code>
                      </div>
                    ))}
                </div>
              </div>
            </div>
            <div className="flex flex-wrap items-center gap-2">
              <Button className="btn-primary" onPress={() => void handleImport()}>
                Run Import
              </Button>
              <Button className="btn-secondary" onPress={() => setIsImportOpen(false)}>
                Close
              </Button>
              <Button className="btn-secondary" onPress={resetImportContext}>
                Reset sample
              </Button>
            </div>
            {importStatus && <div className="import-status">{importStatus}</div>}
          </Dialog>
        </Modal>
      </ModalOverlay>
      <header className="terminal-header">
        <div className="terminal-header__content">
          <div className="logo">
            <span className="logo-text">curlpit</span>
            <span className="logo-version">v1.0</span>
          </div>
          <nav className="header-nav">
            <a href="https://github.com/curlpit-sh/cli" target="_blank" rel="noreferrer">
              GitHub
            </a>
            <a href="#install" className="install-btn">
              Install
            </a>
          </nav>
        </div>
      </header>
      <main className="playground-wrapper">
        <div className="playground">
          <div className="left-panes min-h-0">
            <div className="pane glow">
              <div className="pane-header">
                <div className="window-controls">
                  <div className="window-dot red" />
                  <div className="window-dot yellow" />
                  <div className="window-dot green" />
                  <span className="pane-title">request.curl</span>
                </div>
                <div className="pane-actions">
                  <select
                    className="example-select"
                    value={exampleKey}
                    onChange={(event) => loadExample(event.target.value)}
                  >
                    {playgroundExamples.map((example) => (
                      <option key={example.value} value={example.value}>
                        {example.label}
                      </option>
                    ))}
                  </select>
                  <Button
                    className="btn-secondary"
                    onPress={() => {
                      setImportCommand(importContext.command);
                      setImportStatus(null);
                      setIsImportOpen(true);
                    }}
                  >
                    Import curl
                  </Button>
            </div>
          </div>

          {lastImportSummary && (
            <div className="import-summary border-b border-[var(--terminal-border)] bg-[rgba(10,10,10,0.4)] px-4 py-3">
              {lastImportSummary.split("\n").map((line, index) => (
                <div key={index}>{line}</div>
              ))}
            </div>
          )}

          <div className="editor-wrapper">
            <TextArea
              value={curl}
              onChange={(event) => setCurl(event.target.value)}
                  className="editor"
                  spellCheck={false}
                  aria-label="Curl template editor"
                />
              </div>
            </div>

            <div className="pane">
              <div className="pane-header">
                <div className="window-controls">
                  <div className="window-dot red" />
                  <div className="window-dot yellow" />
                  <div className="window-dot green" />
                  <span className="pane-title">variables.env</span>
                </div>
                <Button className="btn-secondary" onPress={clearVariables}>
                  Clear
                </Button>
              </div>
              <div className="editor-wrapper">
                <TextArea
                  value={variables}
                  onChange={(event) => setVariables(event.target.value)}
                  className="editor"
                  spellCheck={false}
                  aria-label="Variables editor"
                />
              </div>
            </div>
          </div>

          <div className="pane">
            <div className="pane-header">
              <div className="window-controls">
                <div className="window-dot red" />
                <div className="window-dot yellow" />
                <div className="window-dot green" />
                <span className="pane-title">Response</span>
              </div>
              <Button
                className="run-btn"
                onPress={() => void executeRequest()}
                isDisabled={isRunning}
              >
                {isRunning ? "Running…" : "Run"}
              </Button>
            </div>

            <Tabs
              selectedKey={selectedTab}
              onSelectionChange={(key) => setSelectedTab(String(key))}
              className="flex h-full flex-col"
            >
              <TabList aria-label="Response views" className="tab-group flex">
                <Tab
                  id="output"
                  className={({ isSelected }) =>
                    `tab-button${isSelected ? " active" : ""}`
                  }
                >
                  Output
                </Tab>
                <Tab
                  id="headers"
                  className={({ isSelected }) =>
                    `tab-button${isSelected ? " active" : ""}`
                  }
                >
                  Headers
                </Tab>
                <Tab
                  id="raw"
                  className={({ isSelected }) =>
                    `tab-button${isSelected ? " active" : ""}`
                  }
                >
                  Raw
                </Tab>
                <Tab
                  id="export"
                  className={({ isSelected }) =>
                    `tab-button${isSelected ? " active" : ""}`
                  }
                >
                  Export
                </Tab>
              </TabList>

              <TabPanel id="output" className="response-content">
                <div className="response-pane">{renderOutputPanel()}</div>
              </TabPanel>
              <TabPanel id="headers" className="response-content">
                <div className="response-pane">{renderHeadersPanel()}</div>
              </TabPanel>
              <TabPanel id="raw" className="response-content">
                <div className="response-pane">{renderRawPanel()}</div>
              </TabPanel>
              <TabPanel id="export" className="response-content">
                <div className="response-pane space-y-3">
                  <div className="export-controls">
                    <span>Template:</span>
                    <select
                      value={exportTemplate}
                      onChange={(event) => setExportTemplate(event.target.value)}
                    >
                      {EXPORT_TEMPLATES.map((template) => (
                        <option value={template.value} key={template.value}>
                          {template.label}
                        </option>
                      ))}
                    </select>
                    <Button
                      className={`copy-btn${copyState === "copied" ? " copied" : ""}`}
                      onPress={() => void copyToClipboard()}
                    >
                      {copyState === "copied" ? "Copied!" : "Copy snippet"}
                    </Button>
                  </div>
                  <pre className="export-output">{exportSnippet}</pre>
                  {exportStatus && <div className="export-meta">{exportStatus}</div>}
                </div>
              </TabPanel>
            </Tabs>
          </div>
        </div>
      </main>
    </div>
  );
}
