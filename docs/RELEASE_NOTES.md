# Media Compression <VERSION> — unsigned alpha preview

Local image/audio compression with a desktop GUI, presets, custom controls, real-data candidate studies, comparisons and separate exports. Agents use the bundled MCP stdio server or JSON CLI; no development tools, separately installed codecs, account or API key are needed to run the installed application.

This is a public testing preview. macOS 14+ and Windows 11 x64 are intended targets; clean minimum-OS installation/playback, upgrade/uninstall and signing qualification remain incomplete. Installers are unsigned/ad-hoc development builds, not signed/notarized stable releases. Gatekeeper or SmartScreen may require user interaction or block installation. Do not disable system protections. Windows WebView2 publisher/signature qualification remains open; its exact observed provenance and inspection limitations accompany the installer.

## Downloads

| Computer | Installer asset |
|---|---|
<ASSETS_TABLE>

Download the installer for your native architecture and verify it against SHA256SUMS. Mac: mount the DMG and copy the entire app to ~/Applications. Windows: run the per-user installer; it includes offline Evergreen WebView2. The GUI's Agent connection supplies folder-scoped MCP configuration. Read the README's complete setup request to let an agent handle acquisition, installation and MCP/CLI verification. No automatic updater is included; install a later release manually.

## Verification

Exact source commit: `<COMMIT>`. [Native build/test run](<RUN_URL>). All three native jobs must pass before publication: frontend/Rust/real-codec checks, agent interface studies, dependency inspection, installer-copy helper/agent studies and a five-second desktop launch smoke. CI uses macOS 15 and Windows Server 2025 with development tools; these checks do not establish clean macOS 14/Windows 11 behavior or interactive Intel/Windows playback. Evidence files and release-manifest.json accompany the downloads.

Originals remain unchanged; outputs are separately staged and collisions numbered. Every candidate starts from the original. Quality diagnostics inform inspection; they do not guarantee perceptual equivalence, inaudibility, a similarity percentage or an optimal size/quality tradeoff. Agent and GUI studies use separate workspaces.

## Source and notices

Project-authored code is MIT. This software uses [FFmpeg](https://ffmpeg.org/) under LGPL-2.1-or-later, with LAME under its LGPL terms; pngquant/libimagequant are separately executed GPL components. Their licenses are not replaced by MIT. Matching project source, per-target dependency sources/build flags, manifests, notices and license-text review records are attached to this same release. See docs/SOURCE_BUILD.md in the project source for rebuilding and modifying/relinking components. Microsoft's WebView2 retains its own terms. Codec patent/distribution review for a stable release remains open; no codec patent rights are claimed.

Report issues with the release version, OS/architecture and reproducible steps. Do not attach private media or reports containing private local paths without reviewing them.
