# Security policy

## Reporting a vulnerability

Use a [private GitHub security advisory](https://github.com/RlxChap2/rsdownit/security/advisories/new). Include the affected version, reproduction steps and impact. Do not post working exploits, tokens or private URLs in a public issue.

The latest release and the `main` branch receive security fixes.

## Trust boundaries

rsdownit processes untrusted URLs and launches external media tools. The important boundaries are:

- The React view can call only the Tauri commands and dialog capability declared by the app.
- User media URLs are accepted only over HTTP or HTTPS. Literal and DNS-resolved private, loopback, link-local and local-domain targets are rejected. Built-in GET requests pin the checked public addresses and recheck every redirect.
- Direct downloads reject HTML responses and executable or shortcut filename extensions.
- Managed yt-dlp, Deno, and FFmpeg tools are downloaded over HTTPS from publisher locations and installed only after their published SHA-256 matches.
- A `PATH` copy of yt-dlp, Deno, or FFmpeg is trusted as a system-managed tool and is clearly marked as unverified by rsdownit.
- Browser-cookie access, cookie-file selection, and third-party Cobalt requests require an explicit user setting. Cookie contents are not copied into rsdownit settings.
- API tokens are held in memory and removed from the persisted settings copy.

The external yt-dlp process resolves hosts itself after rsdownit validates them. A hostile DNS service can still change its answer in that short interval. Do not use rsdownit as a network service or expose its Tauri command channel to untrusted pages.

## Release integrity

CI runs frontend tests, a production build, Rust formatting, Clippy, Rust tests, npm audit, RustSec audit and dependency review. Release artifacts receive:

- a `SHA256SUMS.txt` manifest;
- a GitHub artifact attestation tied to the workflow and commit;
- optional Authenticode signing when the repository has the certificate secrets configured;
- mandatory Tauri signatures for in-app update packages.

Verify both the checksum and GitHub attestation when possible. Checksums alone do not prove who published a file.

## Antivirus and SmartScreen

No clean-scan count is guaranteed. New or unsigned download utilities can receive reputation warnings because they write files and launch media tools. The project does not use packers, obfuscation or antivirus-evasion techniques.

If a release is flagged:

1. Compare it with the published SHA-256.
2. Verify its GitHub attestation.
3. Check the Authenticode signature when present.
4. Report a suspected compromise privately. Submit a false-positive report to the antivirus vendor only after the integrity checks pass.

## Out of scope

- Problems caused by an unsupported or outdated system copy of yt-dlp or FFmpeg.
- A website changing its extractor behavior without a security impact.
- DRM or paywall bypass requests.
- Downloads performed without permission from the rights holder or service.
