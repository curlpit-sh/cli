# curlpit (npm package)

This package installs the prebuilt curlpit CLI. For full documentation, visit the main project repository at https://github.com/curlpit-sh/cli.

## Install

```bash
npm install --global curlpit
```

The installer downloads the appropriate binary for your current platform (macOS arm64/x64, Linux arm64/x64, Windows x64) from the matching GitHub release. Set `CURLPIT_SKIP_POSTINSTALL=1` to skip downloading or `CURLPIT_LOCAL_BINARY=/path/to/curlpit` to reuse a local build during development. A `tar` binary with `.tar.xz` support must be available on the PATH for Unix platforms.

By default the binary is stored alongside this package in the `dist/` folder. Set `CURLPIT_BIN_DIR` if you need a different location.
