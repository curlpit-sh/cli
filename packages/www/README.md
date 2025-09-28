# Curlpit Web Playground

A minimalistic, terminal-themed interactive landing page for curlpit with a live playground, powered by [Bun's built-in HTML bundler](https://bun.com/docs/bundler/html).

## Features

- **3-Pane Layout**:
  - Top left: `.curl` file editor
  - Bottom left: Environment variables editor
  - Right: Response viewer with tabs (Output, Headers, Raw)

- **Core Functionality**:
  - Parse `.curl` format files
  - Template variable expansion `{VAR_NAME}`
  - Environment variable management
  - Browser-based fetch execution
  - Syntax highlighting for responses
  - Multiple examples (HTTPBin, GitHub API, POST, JSONPlaceholder)
  - Shared Rust templating core compiled to WebAssembly

## Development

```bash
# Install dependencies
bun install

# Start dev server with hot reload
bun dev
# or directly
bun ./index.html

# Build for production
bun build

# Preview production build
bun preview
```

The dev server runs on http://localhost:3000 by default.

## Tech Stack

- **[Bun](https://bun.sh)**: Built-in HTML bundler and dev server
- **[Tailwind CSS](https://tailwindcss.com)**: Via `bun-plugin-tailwind` for utility classes
- **Pure CSS**: Custom terminal theme with CSS variables
- **Vanilla TypeScript**: Framework-free entrypoint (`app.ts`) bundled by Bun
- **Rust ➝ WebAssembly**: `wasm-pack` compiled core parser (`packages/www/curlpit-wasm`)

## Landing Page

- **Hero Section**: Terminal-inspired header with product positioning, GitHub repo link, and install CTA.
- **Three-Pane Playground**: Editors and response viewer mimic the CLI workflow with Tailwind utility classes for layout.
- **Live Examples**: Dropdown seeds the editors with curlpit templates (HTTPBin, GitHub, JSONPlaceholder) to demonstrate templating.
- **Responsive Layout**: CSS grid collapses gracefully for narrower viewports without sacrificing terminal vibes.
- **Bun Bundling**: `bun ./index.html` handles TypeScript + Tailwind, producing a single-page landing experience with instant reloads.

## Project Structure

```
packages/www/
├── index.html      # Main HTML with embedded styles
├── app.ts          # Playground logic (parser, templating, fetch)
├── app.css         # Additional styles and Tailwind import
├── package.json    # Dependencies and scripts
├── bunfig.toml     # Bun configuration with Tailwind plugin
└── dist/           # Production build output
```

## Scripts

- `bun dev` - Start development server with console forwarding
- `bun wasm` - Rebuild the WebAssembly bindings (requires `wasm-pack`)
- `bun build` - Build for production with minification  
- `bun preview` - Preview production build

## Configuration

The Tailwind plugin is configured in `bunfig.toml`:

```toml
[serve.static]
plugins = ["bun-plugin-tailwind"]
```

This allows using Tailwind by simply importing it in CSS:
```css
@import "tailwindcss";
```

Or linking it in HTML:
```html
<link rel="stylesheet" href="tailwindcss" />
```

## Architecture

The playground implements curlpit's core templating logic in TypeScript and WebAssembly:

1. **Parser**: Parses `.curl` format (method, URL, headers, body)
2. **Template Engine**: Rust core compiled to wasm expands `{VARIABLE}` placeholders using environment variables
3. **Executor**: Uses browser's Fetch API to make requests
4. **Response Handler**: Displays formatted responses with syntax highlighting

## Variable Interpolation

The playground prominently displays how curlpit's template variables work:
- Variables like `{API_BASE}` are highlighted in the editor
- Live preview shows which variables will be replaced
- Interpolation details are shown before execution

## CORS Limitations

Due to browser security (CORS), the playground cannot directly call most external APIs. The playground:
- Shows what request would be sent
- Works with CORS-enabled APIs (HTTPBin, JSONPlaceholder)
- Recommends using the actual curlpit CLI for real API testing

## Customization

- **Theme**: Edit CSS variables in the embedded styles
- **Examples**: Add new examples in `app.ts` under the `examples` object
- **Syntax Highlighting**: Enhance the `syntaxHighlightJson()` method

## WebAssembly Module

- Source lives in `packages/www/curlpit-wasm/src/lib.rs`, thin wrapper around `curlpit::web`
- Build updated bindings via `bun wasm` (requires [`wasm-pack`](https://rustwasm.github.io/wasm-pack/installer/))
- Generated assets land in `packages/www/curlpit-wasm/pkg` and are imported by `app.ts`

## Production Build

The production build is optimized with:
- Minified JavaScript and CSS
- Asset hashing for cache busting
- All dependencies bundled

```bash
bun build
# Creates optimized files in dist/
```

## Future Enhancements

For a production version, consider:
- WebAssembly build of curlpit's Rust parser
- Proxy server for CORS bypass
- Monaco Editor for better code editing
- WebWorker for request execution
- LocalStorage for saving requests
- Share functionality with URL encoding
