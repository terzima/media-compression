# Media Compression

A local desktop app for compressing images and audio, inspecting actual candidates, and exporting without changing originals. macOS 14+ (Apple Silicon/Intel) and Windows 11 x64 are first-release targets.

The development GUI implements presets, custom controls, real-data studies, comparisons and safe export. Native CI builds/tests Apple Silicon, Intel Mac and Windows x64, and uploads installers with checksums, source/notices and verification records. Unsigned artifacts are development candidates; signing and clean minimum-OS qualification are deferred. See [implementation milestones](docs/IMPLEMENTATION_PLAN.md) and [platform evidence](docs/RELEASE_QUALIFICATION.md).

## Using the app

1. Add files/folders or drop them into the window.
2. Use Higher quality, Balanced, or Smaller; Advanced exposes the format's supported controls. Reused custom settings remain visible.
3. Compress once or run Quick study. Custom studies accept comma-separated quality/bitrate values and show the number of encodes before starting. Every encode starts from the original; cancellation retains completed candidates.
4. Compare full-resolution images with split, linked zoom/scroll, and transparency backgrounds. Switch audio versions at the backend timeline position, seek, loop, and use a shared volume.
5. Select candidates and choose an export folder. Larger/aggressive outputs can be deliberately exported. Collisions are numbered; folder structure and originals are preserved. Optional JSON reports carry actual settings, bytes, hashes, diagnostics, and errors.

Inputs: static PNG/JPEG/WebP; mono/stereo WAV, FLAC, MP3, AAC/M4A, and Ogg Opus/Vorbis. Outputs: PNG/JPEG/WebP; AAC-LC/M4A, MP3, Ogg Opus, FLAC. Images retain dimensions; JPEG transparency requires an explicit background. Unsupported image precision/color combinations receive an explanation. 16-bit PNG supports lossless optimization. Lossless FLAC requires 16-/24-bit integer sources. Audio preserves supported sample rates/channels; Opus uses 48 kHz and other required conversions are disclosed.

Quality settings are encoder controls, not similarity percentages. SSIM/alpha/timing are diagnostics, not equivalence or inaudibility guarantees. Small images can lack SSIM and still be exported. Video, resizing, surround, AI/cloud services, automatic quality-target selection, and blind statistics are deferred.

The installed application bundles its codecs and needs no account, network, Python, Node, Rust, Homebrew, or separately installed command-line tools. Development builds need the tools below.

## Set it up with your agent

After installing the desktop app, copy and paste this request into your agent. Replace the bracketed folder with the folder containing your media and intended export locations:

```text
Set up Media Compression for this agent using the setup instructions at
https://github.com/terzima/media-compression/blob/codex/desktop-v1/README.md#setup-instructions-for-agents.
Grant access only to [my media folder]. Prefer its local MCP tools; use its
bundled JSON CLI if MCP is unavailable. Load its compression-study guide,
verify the connection, and tell me when it is ready. Preserve my originals.
```

Your agent needs local terminal access to this computer and, for MCP, a client that supports local stdio servers. You do not need to know shell commands. If you leave the folder unspecified, the agent should ask which folder to use. Setup does not start compression or export. During development, these instructions apply to a build containing the agent extension; signed stable installers remain a release gate.

Once setup is complete, try: **“Study this file and show me the smallest candidates with good quality, explain the tradeoffs, and let me compare them before export.”** The agent measures real outputs and refines settings; lossy quality still depends on your requirements and visual/listening feedback.

### Setup instructions for agents

1. Find the installed `media-compression-agent` executable. On macOS, check `/Applications/Media Compression.app/Contents/MacOS/` and `~/Applications/Media Compression.app/Contents/MacOS/`. On Windows, locate the installed GUI via its per-user uninstall record or the user's installation path; `media-compression-agent.exe` is beside the GUI executable. Use the full executable path and argument arrays. If the app or agent helper is missing, explain the missing installation/build and help locate a suitable installer; do not install development tools or replacement codecs as a setup workaround.
2. Run the executable with `--help`, then `guide`, `capabilities` and `tools`. Read the returned study instructions and schemas. No workspace, media import, account, model or API key is needed for these discovery commands.
3. Resolve the folder specified by the user and run `config --root <absolute-folder>`. Grant only the requested folders; repeat `--root` when multiple folders are authorized. If no folder was identified, ask for it. The GUI's **Agent connection → Choose granted folder → Copy configuration** provides the equivalent configuration and the bundled skill path.
4. If this client supports local MCP configuration, add the generated executable/arguments using its documented configuration format, preserve existing servers, and load or install the bundled `media-compression` skill through the client's supported mechanism when available. The MCP server already includes the skill in its instructions. Keep the server local over stdio. Do not add cloud services, credentials, or arbitrary folders. Restart/reload the connection if the client requires it, and distinguish that step from completed setup.
5. Verify MCP tool discovery and call `compression_capabilities`; check that the returned granted folders match the user's choice. If this client cannot configure/reload MCP, use the full-path JSON CLI and its embedded `guide` instead. Verify `capabilities` and `tools`, and disclose that the terminal fallback is active. Never claim MCP is connected just because a configuration file was written.
6. Report the active interface and granted folders. Keep setup separate from media processing. For a later compression request, use the guide to run real candidate studies from originals, show exact bytes/settings/diagnostics and meaningful tradeoffs, and export only the selections authorized by the user. Agent and GUI studies use separate workspaces; outputs can be opened normally.

Both interfaces use the same engine and bundled codecs. The connected agent has its own model/data policies; this app supplies local compression tools. See [tool schemas, JSON requests and workspace details](docs/AGENT_INTERFACE.md). The GUI remains independently usable.

## Development

Install Node 24.21.0 and Rust 1.99.0. macOS needs Xcode command-line tools; Windows needs Visual Studio C++ tools, MSYS2 MinGW GCC/binutils/make. Install CMake 4.4.4 and Ninja 1.13.2 (for example in a build-only Python venv). Python is never bundled.

```sh
npm ci
npm run codecs
npm run worker
npm run check
cargo test --locked --workspace
cargo test --locked -p media-engine -- --include-ignored
cargo test --locked -p media-agent -- --include-ignored
cargo clippy --locked --workspace --all-targets -- -D warnings
npm run desktop:dev
```

`npm run codecs` verifies pinned source archives, compiles helpers, verifies their launch, and stages them under src-tauri/resources/codecs. It generates patched upstream bindings without modifying registry source. On Windows run this from a native terminal with MSYS2 make/sh and MinGW tools on PATH. Rust-linked C bindings use MSVC. Read [dependencies](docs/DEPENDENCIES.md) for pins, licenses, build flags, provenance and source material.

```sh
npm run sources
npm run desktop:build -- --bundles dmg  # Mac, separate native builds per architecture
npm run desktop:build -- --bundles nsis # Windows, offline WebView2 included
node scripts/generate-fixtures.mjs     # generated demonstration files
```

The desktop build wrapper defaults to CI-style DMG layout so it does not require Finder automation permission. Clear `.tools/native`, `.tools/patched-sys`, and staged helpers before auditing a fully clean codec rebuild. Lockfiles are committed; helpers/cache/artifacts are ignored. Build results do not substitute for signed clean-machine installation evidence.

See [architecture](docs/ARCHITECTURE.md), [provenance](docs/PROVENANCE.md), and [project brief](docs/PROJECT_BRIEF.md).

## License

Project-authored code is MIT. Dependencies retain their licenses. In particular pngquant/libimagequant are GPL components distributed as a separate executable with corresponding source; FFmpeg/LAME retain LGPL obligations. Each qualified release must include notices, matching source/build material, and checksums.
