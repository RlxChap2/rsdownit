# Contributing

Keep pull requests focused and explain the user-visible behavior. New site-specific extraction logic should normally go upstream to yt-dlp instead of being duplicated here.

Before opening a pull request:

```powershell
pnpm install --frozen-lockfile
pnpm test
pnpm test:smoke
pnpm build

Set-Location src-tauri
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

Do not commit media, cookies, API tokens, downloaded executables, certificates or private URLs. Use a private security advisory for vulnerabilities.

By contributing, you agree that your contribution may be licensed under the MIT License.
