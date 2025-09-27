# Curlpit VS Code Extension

Adds basic syntax highlighting for Curlpit `.curl` request files.

## Usage

1. Copy or symlink this folder (`dist/vscode-extension`) into your VS Code extensions directory, e.g. `~/.vscode/extensions/curlpit`.
2. Restart VS Code.
3. Open a `.curl` file. The language mode should switch to **Curlpit** automatically.

The extension highlights HTTP methods, headers, URLs, and templated variables wrapped in `{...}` or `{{...}}`.
