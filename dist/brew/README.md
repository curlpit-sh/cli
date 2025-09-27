# curlpit Homebrew Formula

Use `curlpit.rb` as the formula for your tap (e.g. `curlpit-sh/homebrew-curlpit`). Update the `version`, `url`, and `sha256` fields for each release.

Quick publish steps:

1. Compute tarball checksum:
   ```bash
   shasum -a 256 curlpit-0.1.0.tar.gz
   ```
2. Update `curlpit.rb` with version/tag/sha.
3. Commit and push to your tap repository.
4. Users install via:
  ```bash
  brew tap curlpit-sh/curlpit
  brew install curlpit
  ```

You can integrate this into the release workflow by copying the updated formula into the tap repo and pushing automatically.
