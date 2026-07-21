# rsdownit

![rsdownit workbench](docs/screenshot.png)

rsdownit is a Windows-first desktop downloader for public video and audio. Paste a link, choose a format and folder, then let the local engine resolve it.

The project does not promise every website. Site markup, authentication and extractor support change. DRM, paywall bypass and private-network URLs are intentionally unsupported.

## What works

- Video, muted video and audio-only downloads.
- Best quality or a 2160p–360p video cap.
- Native audio, MP3, M4A, Opus or WAV, with optional bitrate selection.
- Native output-folder picker. Existing files are never overwritten.
- Live progress, speed, ETA, cancel, retry and local history.
- Optional browser cookies for media the user is allowed to access.
- Optional self-hosted Cobalt fallback. Community servers are off by default.
- Light and dark themes. No telemetry.
- Signed in-app updates with install-now and later options.
- Automatic update checks for the managed yt-dlp and FFmpeg tools.

Windows 10 and 11 are the current release targets. Tauri keeps the UI and Rust core portable; managed FFmpeg provisioning for macOS and Linux is still planned.

## Resolver order

| Order | Resolver | Purpose |
| --- | --- | --- |
| 1 | Direct stream | Saves a public media file without another process |
| 2 | yt-dlp | Local extractor with broad site support; HLS and DASH use native fragment concurrency |
| 3 | Self-hosted Cobalt | Optional endpoint supplied by the user |
| 4 | Community Cobalt | Explicit opt-in; sends the media URL to a third party |
| 5 | HTML probe | Looks for public `og:video`, `<video>`, `<source>` and media links |

Managed `yt-dlp` and Windows FFmpeg downloads come from their publisher URLs. rsdownit verifies the published SHA-256 before installing either tool. A system copy on `PATH` can also be used, but is reported as system-provided rather than publisher-verified.

## Privacy and safety

- Normal downloads stay local unless an API fallback is enabled.
- API tokens are kept for the current process and are not written to `settings.json`.
- Browser cookies are opt-in and passed directly to local `yt-dlp`.
- `file:`, embedded credentials, localhost, private IP ranges and common local domains are blocked for media URLs.
- Executable and shortcut filename extensions are blocked in direct downloads.
- Tauri runs with a restrictive CSP and a small capability set.

Read [SECURITY.md](SECURITY.md) before distributing binaries.

## Build on Windows

Requirements: Node.js 20+, pnpm 11, stable Rust, Microsoft C++ Build Tools and WebView2.

```powershell
pnpm install --frozen-lockfile
pnpm tauri dev
pnpm tauri build
```

Checks used by CI:

```powershell
pnpm test
pnpm test:smoke
pnpm build
pnpm audit --prod

Set-Location src-tauri
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

Three network integration tests are ignored by default. Run them only when real downloads are acceptable:

```powershell
Set-Location src-tauri
cargo test -- --ignored
```

## Release verification

Tagged builds can publish signed in-app updates. Add the updater private key to the `TAURI_SIGNING_PRIVATE_KEY` GitHub Actions secret and its password, when used, to `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`. Never commit the private key. The public key is already pinned in `src-tauri/tauri.conf.json`. A tag must match the package version, for example `v0.1.0`.

GitHub Actions publishes installers, the portable executable and `SHA256SUMS.txt`. Compare a downloaded file locally:

```powershell
Get-FileHash .\rsdownit.exe -Algorithm SHA256
Get-Content .\SHA256SUMS.txt
```

For a public GitHub release, verify the signed build provenance:

```powershell
gh attestation verify .\rsdownit.exe --repo RlxChap2/rsdownit
```

Unsigned local builds may trigger Windows reputation warnings. A checksum proves file identity, not publisher identity; production releases should also use Authenticode signing. The workflow supports an optional `WINDOWS_CERTIFICATE` base64 PFX secret and `WINDOWS_CERTIFICATE_PASSWORD`. App updates are separately verified with Tauri's required update signature.

## Project layout

```text
src/app/                     App state and composition
src/components/              Shared layout and UI controls
src/features/downloads/      Download form, setup status and history
src/features/settings/       Settings dialog
src/features/updates/        Signed update prompt
src/styles/                  Fonts, design tokens and application styles
src-tauri/src/downloader.rs  Provider chain, progress and cancellation
src-tauri/src/providers/     Direct, yt-dlp and Cobalt adapters
src-tauri/src/security.rs    URL and filename policy
src-tauri/src/tools.rs       Tool discovery, download and SHA-256 verification
tests/ui-smoke.mjs           Playwright desktop and responsive smoke test
```

## Legal

Download only material you own or are permitted to save. Site terms and copyright law still apply. rsdownit does not bypass DRM or paywalls.

## License

rsdownit is available under the [MIT License](LICENSE). External tools have separate terms; see [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).
