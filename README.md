# curlpit

Terminal-first HTTP runner that executes scripted requests, prints structured output, and writes response bodies to disk.

## Highlights

- **File-first workflow** – store HTTP requests as `.curl` files (Curlpit syntax or imported curl commands) and version them alongside code.
- **Live response capture** – pretty CLI output with colored status, headers, previews, and automatic response body archiving by request name.
- **Profile-aware environments** – load `curlpit.json` plus `.env` files, merge variables across profiles, and reuse placeholders like `{API_BASE}`.
- **Interactive mode** – browse `.curl` files, switch profiles, import curl commands, and now scaffold new projects with a demo request and config.
- **Template exports** – transform requests into code snippets (e.g., JS fetch) with configurable templates.
- **Robust importer** – convert complex curl invocations into Curlpit requests, handling header rules, placeholder substitution, and env variables.

## Installation

### Using npm (prebuilt binaries)

```bash
npm install --global curlpit
```

The installer downloads a platform-specific binary from the matching GitHub release. To use a locally built executable during development, set `CURLPIT_LOCAL_BINARY=/path/to/curlpit` before running `npm install`. To skip downloads entirely, set `CURLPIT_SKIP_POSTINSTALL=1`.

### Using Deno

```bash
deno run -A https://raw.githubusercontent.com/curlpit-sh/cli/main/packages/deno/src/index.ts
```

Environment variables such as `CURLPIT_VERSION`, `CURLPIT_BIN_DIR`, and `CURLPIT_SKIP_CHECKSUM` mirror the npm installer. The script expects `tar` (plus `unzip` or PowerShell on Windows) to be available on the PATH.

### Homebrew tap

Copy `packages/brew/curlpit.rb` into your tap repository and update the version, tag URL, and SHA256 for each release. Users can then run:

```bash
brew tap curlpit-sh/curlpit
brew install curlpit
```

### From source

```bash
cargo install --path .
```

## Usage

```bash
curlpit examples/httpbin-get.curl --preview 200
```

Use `curlpit --help` for the full list of options.

## Releases

GitHub releases provide binaries for macOS (arm64, x64), Linux (arm64, x64), and Windows (x64). Checksums accompany each asset for verification. npm and Deno installers consume the same artifacts via install-time downloads. Ensure `tar` on your system supports `.tar.xz` archives (default on modern macOS/Linux/Windows).

### Releasing

Releases are orchestrated by [`cargo-dist`](https://github.com/axodotdev/cargo-dist). Bump the crate version, update any generated installers/templates under `packages/`, commit, and then push a tag like `v0.2.0`. The `Release` GitHub workflow builds platform artifacts, generates shell/PowerShell installers, and publishes the GitHub release automatically. To preview the matrix locally run:

```bash
cargo dist plan --output-format=json
```

## Development

1. Ensure Rust (stable) is installed.
2. Run `cargo fmt`, `cargo clippy`, and `cargo test` before opening a PR.
3. For npm packaging, everything lives under `packages/npm`; keep the release artifacts in sync with `packages/npm/scripts/install.js` mappings and run `npm publish` from that directory when releasing to the registry.
4. Deno and Homebrew assets live under `packages/deno` and `packages/brew`. Update these when cutting a release. The landing page lives in `packages/www`.

Automated workflows (`.github/workflows`) handle CI checks and release packaging/tagging.
