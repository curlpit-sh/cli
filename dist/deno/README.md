# curlpit Deno Installer

Use the `install.ts` script to download the curlpit binary for the current platform directly from GitHub releases.

```bash
deno run -A https://raw.githubusercontent.com/curlpit-sh/cli/main/dist/deno/install.ts
```

Requires a `tar` binary with `.tar.xz` support (present on modern macOS/Linux and Windows).

Environment overrides:

- `CURLPIT_VERSION` — tag to install (default: latest published version, update script before releasing).
- `CURLPIT_REPOSITORY` — GitHub repo slug if you are testing forks.
- `CURLPIT_PLATFORM`, `CURLPIT_ARCH` — override detected os/arch.
- `CURLPIT_BIN_DIR` — destination directory (defaults to `~/.local/bin` on Unix, `%USERPROFILE%/AppData/Local/curlpit/bin` on Windows).
- `CURLPIT_SKIP_CHECKSUM` — skip SHA-256 verification (not recommended).
- `CURLPIT_LOCAL_BINARY` — not applicable; use manual copy instead.

The installer requires: `--allow-env --allow-net --allow-run --allow-read --allow-write`.
