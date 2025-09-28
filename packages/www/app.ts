import initWasm, {
  process_request as wasmProcessRequest,
} from "./curlpit-wasm/pkg/curlpit_wasm.js";
import wasmAssetUrl from "./curlpit-wasm/pkg/curlpit_wasm_bg.wasm";

type TabKey = "output" | "headers" | "raw";

type EnvVars = Record<string, string>;

interface PlaygroundExample {
  curl: string;
  variables: string;
}

interface PlaygroundResponse {
  status: number;
  statusText: string;
  headers: Record<string, string>;
  body: string;
}

interface WasmHeader {
  name: string;
  value: string;
}

interface WasmRequest {
  method: string;
  url: string;
  headers: WasmHeader[];
  body: string | null | undefined;
}

interface WasmInterpolationDetail {
  key: string;
  value: string;
}

interface WasmProcessedRequest {
  request: WasmRequest;
  interpolation: WasmInterpolationDetail[];
}

const ensureWasmReady = (() => {
  let initPromise: Promise<void> | null = null;
  return () => {
    if (!initPromise) {
      initPromise = initWasm({ module_or_path: wasmAssetUrl }).then(
        () => undefined,
      );
    }
    return initPromise;
  };
})();

class CurlpitPlayground {
  private readonly curlEditor: HTMLTextAreaElement;
  private readonly variablesEditor: HTMLTextAreaElement;
  private readonly responseOutput: HTMLElement;
  private readonly runBtn: HTMLButtonElement;
  private readonly clearVarsBtn: HTMLButtonElement;
  private readonly exampleSelector: HTMLSelectElement;
  private readonly tabs: NodeListOf<HTMLButtonElement>;
  private currentTab: TabKey;
  private lastResponse: PlaygroundResponse | null;
  private readonly examples: Record<string, PlaygroundExample>;

  constructor() {
    const curlEditor = document.getElementById("curl-editor");
    const variablesEditor = document.getElementById("variables-editor");
    const responseOutput = document.getElementById("response-output");
    const runBtn = document.getElementById("run-btn");
    const clearVarsBtn = document.getElementById("clear-vars");
    const exampleSelector = document.getElementById("example-selector");
    const tabs = document.querySelectorAll<HTMLButtonElement>(".tab");

    if (!(curlEditor instanceof HTMLTextAreaElement)) {
      throw new Error('Missing textarea with id "curl-editor"');
    }
    if (!(variablesEditor instanceof HTMLTextAreaElement)) {
      throw new Error('Missing textarea with id "variables-editor"');
    }
    if (!(responseOutput instanceof HTMLElement)) {
      throw new Error("Missing response output element");
    }
    if (!(runBtn instanceof HTMLButtonElement)) {
      throw new Error("Missing run button element");
    }
    if (!(clearVarsBtn instanceof HTMLButtonElement)) {
      throw new Error("Missing clear variables button element");
    }
    if (!(exampleSelector instanceof HTMLSelectElement)) {
      throw new Error("Missing example selector element");
    }

    this.curlEditor = curlEditor;
    this.variablesEditor = variablesEditor;
    this.responseOutput = responseOutput;
    this.runBtn = runBtn;
    this.clearVarsBtn = clearVarsBtn;
    this.exampleSelector = exampleSelector;
    this.tabs = tabs;
    this.currentTab = "output";
    this.lastResponse = null;
    this.examples = {
      httpbin: {
        curl: `# GET request with templated variables (like curlpit)
GET {API_BASE}/get
User-Agent: {USER_AGENT}
Accept: application/json
Authorization: Bearer {API_TOKEN}`,
        variables: `# Environment variables (these get interpolated)
API_BASE=https://httpbin.org
USER_AGENT=curlpit-playground/1.0
API_TOKEN=demo-token-12345`,
      },
      github: {
        curl: `# GitHub API with variable expansion
GET {API_BASE}/repos/{OWNER}/{REPO}
Accept: application/vnd.github.v3+json
User-Agent: {USER_AGENT}
Authorization: Bearer {GITHUB_TOKEN}`,
        variables: `# GitHub API configuration
API_BASE=https://api.github.com
OWNER=curlpit-sh
REPO=cli
USER_AGENT=curlpit/1.0
GITHUB_TOKEN=ghp_your_token_here`,
      },
      post: {
        curl: `# POST with JSON body and variables
POST {API_BASE}/posts
Content-Type: application/json
Accept: application/json
X-Request-ID: {REQUEST_ID}

{
  "title": "{POST_TITLE}",
  "body": "Posted via curlpit at {TIMESTAMP}",
  "userId": {USER_ID}
}`,
        variables: `# JSONPlaceholder POST example
API_BASE=https://jsonplaceholder.typicode.com
POST_TITLE=Test Post from Curlpit
USER_ID=1
REQUEST_ID=req_${Date.now()}
TIMESTAMP=${new Date().toISOString()}`,
      },
      placeholder: {
        curl: `# JSONPlaceholder API (CORS-friendly)
GET {BASE_URL}/users/{USER_ID}
Accept: application/json
User-Agent: {APP_NAME}/{APP_VERSION}
X-Custom-Header: {CUSTOM_VALUE}`,
        variables: `# Works in browser (CORS-enabled API)
BASE_URL=https://jsonplaceholder.typicode.com
USER_ID=1
APP_NAME=curlpit-playground
APP_VERSION=1.0.0
CUSTOM_VALUE=demo-${Math.random().toString(36).substring(7)}`,
      },
    };

    this.init();
  }

  private init(): void {
    this.runBtn.addEventListener("click", () => void this.executeRequest());
    this.clearVarsBtn.addEventListener("click", () => {
      this.clearVariables();
      void this.showInterpolationPreview();
    });
    this.exampleSelector.addEventListener("change", (event) => {
      const target = event.target as HTMLSelectElement | null;
      if (target) {
        this.loadExample(target.value);
      }
    });

    this.tabs.forEach((tabElement) => {
      tabElement.addEventListener("click", () => {
        const targetTab = tabElement.dataset.tab as TabKey | undefined;
        if (targetTab) {
          this.switchTab(targetTab);
        }
      });
    });

    document
      .querySelectorAll<HTMLButtonElement>(".copy-btn")
      .forEach((button) => {
        button.addEventListener("click", (event) => {
          const target = event.currentTarget;
          if (target instanceof HTMLElement) {
            void this.copyToClipboard(target);
          }
        });
      });

    this.curlEditor.addEventListener("input", () => {
      this.highlightSyntax();
      void this.showInterpolationPreview();
    });

    this.variablesEditor.addEventListener("input", () => {
      void this.showInterpolationPreview();
    });

    document.addEventListener("keydown", (event) => {
      if ((event.metaKey || event.ctrlKey) && event.key === "Enter") {
        event.preventDefault();
        void this.executeRequest();
      }
    });

    this.highlightSyntax();
    void this.showInterpolationPreview();
  }

  private async executeRequest(): Promise<void> {
    const curlContent = this.curlEditor.value;
    const varsContent = this.variablesEditor.value;

    this.lastResponse = null;
    this.setResponse("output", '<span class="dim">Preparing request...</span>');
    this.runBtn.textContent = "Running...";
    this.runBtn.disabled = true;

    try {
      const vars = this.parseEnvVariables(varsContent);
      const processed = await this.processWithWasm(curlContent, vars);
      const request = processed.request;

      if (!request.url || !/^https?:\/\//i.test(request.url)) {
        throw new Error("Invalid URL. Must start with http:// or https://");
      }

      const headers: Record<string, string> = {};
      for (const header of request.headers) {
        headers[header.name] = header.value;
      }

      const fetchOptions: RequestInit = {
        method: request.method,
        headers,
      };

      if (
        request.body &&
        ["POST", "PUT", "PATCH", "DELETE"].includes(request.method)
      ) {
        fetchOptions.body = request.body;
      }

      const timestamp = new Date().toISOString();
      let output = `<span class="dim">[${timestamp}]</span>\n`;
      output += `<span class="syntax-method">${request.method}</span> ${this.escapeHtml(request.url)}\n`;

      if (processed.interpolation.length > 0) {
        output += '<span class="dim">Variable Interpolation:</span>\n';
        output += this.renderInterpolationDetails(processed.interpolation);
      }
      output += "\n";

      try {
        await ensureWasmReady();
        const response = await fetch(request.url, fetchOptions);

        this.lastResponse = {
          status: response.status,
          statusText: response.statusText,
          headers: Object.fromEntries(response.headers.entries()),
          body: await response.text(),
        };

        const statusClass = response.ok ? "status-success" : "status-error";
        output += `<span class="${statusClass}">HTTP ${response.status} ${response.statusText}</span>\n\n`;

        try {
          const json = JSON.parse(this.lastResponse.body);
          output += this.syntaxHighlightJson(json);
        } catch {
          output += this.escapeHtml(this.lastResponse.body);
        }
      } catch (fetchError) {
        const errorMessage =
          fetchError instanceof Error ? fetchError.message : String(fetchError);
        output += `<span class="status-error">Request failed: ${this.escapeHtml(errorMessage)}</span>\n\n`;

        if (
          errorMessage.includes("CORS") ||
          errorMessage.includes("Failed to fetch")
        ) {
          output +=
            '<span class="dim">Note: Browser security (CORS) prevents direct API calls to most external domains.\n';
          output += "To test real APIs:\n";
          output += "  1. Use the curlpit CLI: npm install -g curlpit\n";
          output += "  2. Use a CORS proxy service\n";
          output +=
            "  3. Test with APIs that allow CORS (HTTPBin, JSONPlaceholder)</span>\n\n";

          output +=
            '<span class="dim">--- Request that would be sent ---</span>\n';
          output += `<span class="syntax-method">${request.method}</span> ${this.escapeHtml(request.url)}\n`;
          for (const header of request.headers) {
            output += `<span class="header-key">${header.name}:</span> <span class="header-value">${this.escapeHtml(header.value)}</span>\n`;
          }
          if (request.body) {
            output += "\n" + this.escapeHtml(request.body);
          }
        }
      }

      this.setResponse("output", output);
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      this.setResponse(
        "output",
        `<span class="status-error">Error: ${this.escapeHtml(message)}</span>`,
      );
    } finally {
      this.runBtn.textContent = "Run ▶";
      this.runBtn.disabled = false;
    }
  }

  private setResponse(tab: TabKey, content: string): void {
    if (tab === "headers" && this.lastResponse) {
      let output = '<span class="dim">Response Headers:</span>\n\n';
      Object.entries(this.lastResponse.headers).forEach(([key, value]) => {
        output += `<span class="header-key">${key}:</span> <span class="header-value">${this.escapeHtml(value)}</span>\n`;
      });
      this.responseOutput.innerHTML = output;
      return;
    }

    if (tab === "raw" && this.lastResponse) {
      this.responseOutput.innerHTML = `<span class="dim">Raw Response Body:</span>\n\n${this.escapeHtml(this.lastResponse.body)}`;
      return;
    }

    this.responseOutput.innerHTML = content;
  }

  private switchTab(tab: TabKey): void {
    this.currentTab = tab;
    this.tabs.forEach((tabElement) => tabElement.classList.remove("active"));
    const activeTab = document.querySelector(`[data-tab="${tab}"]`);
    if (activeTab instanceof HTMLElement) {
      activeTab.classList.add("active");
    }
    this.setResponse(tab, this.responseOutput.innerHTML);
  }

  private loadExample(exampleKey: string): void {
    const example = this.examples[exampleKey];
    if (!example) {
      return;
    }

    this.curlEditor.value = example.curl;
    this.variablesEditor.value = example.variables;
    this.highlightSyntax();
    void this.showInterpolationPreview();
  }

  private clearVariables(): void {
    this.variablesEditor.value = "# Environment variables\n";
  }

  private highlightSyntax(): void {
    // Placeholder for future syntax highlighting implementation.
  }

  private syntaxHighlightJson(obj: unknown): string {
    const json = JSON.stringify(obj, null, 2);
    return this.escapeHtml(json)
      .replace(/"([^"\\]+)":/g, '<span class="syntax-header-key">"$1"</span>:')
      .replace(
        /: "([^"\\]*)"/g,
        ': <span class="syntax-header-value">"$1"</span>',
      )
      .replace(
        /: (\d+(?:\.\d+)?)/g,
        ': <span class="syntax-variable">$1</span>',
      )
      .replace(
        /: (true|false|null)/g,
        ': <span class="syntax-method">$1</span>',
      );
  }

  private escapeHtml(text: string): string {
    const div = document.createElement("div");
    div.textContent = text;
    return div.innerHTML;
  }

  private renderInterpolationDetails(
    details: WasmInterpolationDetail[],
  ): string {
    if (details.length === 0) {
      return "";
    }

    let html = '\n<div class="interpolation-info">';
    details.forEach((detail) => {
      html += '<div class="interpolation-line">';
      html += `<span class="variable-highlight">{${this.escapeHtml(detail.key)}}</span>`;
      html += '<span class="arrow">→</span>';
      html += `<span class="syntax-header-value">${this.escapeHtml(detail.value)}</span>`;
      html += '</div>\n';
    });
    html += '</div>\n';

    return html;
  }

  private parseEnvVariables(content: string): EnvVars {
    const vars: EnvVars = {};
    const lines = content.split("\n");

    for (const line of lines) {
      const trimmed = line.trim();

      if (!trimmed || trimmed.startsWith("#")) {
        continue;
      }

      const match = trimmed.match(/^([A-Z_][A-Z0-9_]*)\s*=\s*(.*)$/);
      if (!match) {
        continue;
      }

      const [, key, value] = match;

      if (value.includes("${") && value.includes("}")) {
        try {
          const evaluated = value.replace(/\$\{([^}]+)\}/g, (_, expr) => {
            if (expr === "Date.now()") {
              return String(Date.now());
            }
            if (expr === "Math.random()") {
              return String(Math.random());
            }
            return expr;
          });
          vars[key] = evaluated;
        } catch (err) {
          console.error("Failed to evaluate variable expression", err);
          vars[key] = value;
        }
      } else {
        vars[key] = value;
      }
    }

    return vars;
  }

  private async processWithWasm(
    curlContent: string,
    vars: EnvVars,
  ): Promise<WasmProcessedRequest> {
    await ensureWasmReady();
    return wasmProcessRequest(curlContent, vars) as WasmProcessedRequest;
  }

  private async showInterpolationPreview(): Promise<void> {
    const curlContent = this.curlEditor.value;
    const varsContent = this.variablesEditor.value;

    try {
      const vars = this.parseEnvVariables(varsContent);
      const processed = await this.processWithWasm(curlContent, vars);

      let output =
        '<span class="dim">Ready to run. Press "Run" or Cmd/Ctrl+Enter to execute.</span>\n\n';

      if (processed.interpolation.length > 0) {
        output +=
          '<span class="dim">Template variables will be interpolated:</span>\n';
        output += this.renderInterpolationDetails(processed.interpolation);
        output += "\n";
        output += '<span class="dim">Resulting URL:</span>\n';
        output += `  <span class="syntax-url">${this.escapeHtml(processed.request.url)}</span>`;
      }

      this.responseOutput.innerHTML = output;
    } catch (error) {
      // Silently ignore errors in preview to avoid noisy UX.
      console.warn("Preview failed", error);
    }
  }

  private async copyToClipboard(button: HTMLElement): Promise<void> {
    const textToCopy = button.dataset.copy;
    if (!textToCopy || !navigator.clipboard) {
      return;
    }

    try {
      await navigator.clipboard.writeText(textToCopy);
      const originalText = button.textContent ?? "";
      button.textContent = "Copied!";
      button.classList.add("copied");
      window.setTimeout(() => {
        button.textContent = originalText;
        button.classList.remove("copied");
      }, 2000);
    } catch (err) {
      console.error("Failed to copy:", err);
    }
  }
}

document.addEventListener("DOMContentLoaded", () => {
  void ensureWasmReady().then(() => {
    new CurlpitPlayground();
  });
});
