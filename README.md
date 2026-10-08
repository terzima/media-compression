# Media Compression

A local desktop app for compressing images and audio, inspecting actual candidates, and exporting without changing originals. macOS 14+ (Apple Silicon/Intel) and Windows 11 x64 are first-release targets.

The GUI implements presets, custom controls, real-data studies, comparisons and safe export. [Download the alpha preview](https://github.com/terzima/media-compression/releases/tag/v0.1.0-alpha.1) for testing, with SHA256SUMS, matching source/notices and exact build evidence. Native CI builds/tests Apple Silicon, Intel Mac and Windows x64 before publication. Preview installers are unsigned development candidates; signing and clean minimum-OS qualification are deferred. See [implementation milestones](docs/IMPLEMENTATION_PLAN.md) and [platform evidence](docs/RELEASE_QUALIFICATION.md).

This software uses FFmpeg under LGPL-2.1-or-later and separately executed pngquant/libimagequant under GPL terms. Their matching source is attached to each release. See [source rebuilding instructions](docs/SOURCE_BUILD.md); MIT licensing applies to project-authored code.

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

Copy and paste this request into an agent with local terminal and download access. It works whether or not you already have the app. Replace the bracketed folder with the folder containing your media and intended export locations, or let the agent ask for it:

```text
Download, install if needed, and set up Media Compression for this agent.
Official project: https://github.com/terzima/media-compression
Installers: https://github.com/terzima/media-compression/releases
First testing preview: https://github.com/terzima/media-compression/releases/tag/v0.1.0-alpha.1
My media/export folder: [my media folder; ask me if unspecified].

Detect my OS and CPU architecture. Reuse an installed app if its bundled
media-compression-agent works. Otherwise download the matching official
installer and verify its published SHA-256 before installing per-user.
For testing, I accept a published unsigned alpha preview. Obtain the
matching installer, SHA256SUMS and release-manifest.json from the official
release assets. Identify it as a development preview. If no suitable
release is available or the OS requires my interaction, explain the
exact remaining step. Do not
disable OS protections or install development tools or replacement codecs.

Find the bundled agent executable and read --help, guide, capabilities
and tools. Grant only my specified folders using config --root. Prefer
local stdio MCP: preserve existing client servers, load the bundled study
skill where supported, and verify tool discovery and compression_capabilities.
If MCP cannot be configured or reloaded here, use the full-path JSON CLI
and embedded guide, and verify capabilities and tools. Report the installed
version/build, executable path, active interface and granted folders.
Do not compress or export during setup; preserve my originals.
```

You do not need to know shell commands or have the app downloaded first. Downloading needs internet access; installed compression runs locally. MCP needs a client that supports local stdio servers; terminal access is sufficient for the CLI fallback. An agent that cannot access your computer cannot install or run local software. Login, native permission dialogs, or a client reload may require your interaction; the agent should complete the available steps and explain what remains.

**Preview distribution:** public alpha release assets can be downloaded without GitHub sign-in. They are unsigned test candidates, with signing and clean minimum-OS qualification still deferred. The request explicitly accepts these for testing; replace that paragraph with a stable-only requirement if you prefer to wait for qualification. It must never claim setup succeeded when no suitable installer could be obtained.

Once setup is complete, try: **“Help me make this image smaller. Ask only what you need, then run it and give me the copy.”** Provide the image and an output folder. The agent should clarify whether you want exactly unchanged pixels or allow visually similar lossy copies, then perform the authorized work. You do not need to pick codecs or learn commands. Your existing agent supplies the conversation; the app supplies local tools, with no built-in AI model or separate subscription. Read [copyable requests, measured-study results and the GUI walkthrough](docs/USER_GUIDE.md). This conversational instruction update and inline GUI help require a development build after alpha.1; the installed alpha can still run studies with an explicit request.

### Setup instructions for agents

1. Detect the native OS/architecture and look for an installed `media-compression-agent`. On macOS, check `/Applications/Media Compression.app/Contents/MacOS/` and `~/Applications/Media Compression.app/Contents/MacOS/`. On Windows, locate the GUI via its per-user uninstall record or the user's installation path; `media-compression-agent.exe` is beside it. Test the helper's discovery commands before replacing a working copy. If the app/helper is missing, follow [installer acquisition](#installer-acquisition-for-agents), then resume discovery. Use full executable paths and argument arrays. Do not build from source, install developer tools, or add replacement codecs as an end-user setup workaround.
2. Run the executable with `--help`, then `guide`, `capabilities` and `tools`. Read the returned study instructions and schemas. No workspace, media import, account, model or API key is needed for these discovery commands.
3. Resolve the folder specified by the user and run `config --root <absolute-folder>`. Grant only the requested folders; repeat `--root` when multiple folders are authorized. If no folder was identified, ask for it. The GUI's **Agent connection → Choose granted folder → Copy configuration** provides the equivalent configuration and the bundled skill path.
4. If this client supports local MCP configuration, add the generated executable/arguments using its documented configuration format, preserve existing servers, and load or install the bundled `media-compression` skill through the client's supported mechanism when available. The MCP server already includes the skill in its instructions. Keep the server local over stdio. Do not add cloud services, credentials, or arbitrary folders. Restart/reload the connection if the client requires it, and distinguish that step from completed setup.
5. Verify MCP tool discovery and call `compression_capabilities`; check that the returned granted folders match the user's choice. If this client cannot configure/reload MCP, use the full-path JSON CLI and its embedded `guide` instead. Verify `capabilities` and `tools`, and disclose that the terminal fallback is active. Never claim MCP is connected just because a configuration file was written.
6. Report the active interface and granted folders. Keep setup separate from media processing. For a later compression request, use the guide to run real candidate studies from originals, show exact bytes/settings/diagnostics and meaningful tradeoffs, and export only the selections authorized by the user. Agent and GUI studies use separate workspaces; outputs can be opened normally.

Both interfaces use the same engine and bundled codecs. The connected agent has its own model/data policies; this app supplies local compression tools. See [tool schemas, JSON requests and workspace details](docs/AGENT_INTERFACE.md). The GUI remains independently usable.

**Automatic folder workflow (development extension after alpha.1):** “Compress this folder losslessly,” “Make smaller copies of this folder,” or “Create both Lossless and Smaller copies” map to one `compress_folder` MCP call or `compress-folder <folder> --mode lossless|smaller|both --root <granted-folder>`. The app chooses deterministic presets, verifies and exports outputs, reuses its cache, excludes generated folders, and returns sizes, savings, absolute output paths and errors. No per-file agent script or exhaustive study is required. Lossless is the safe default; Smaller explicitly permits lossy compression. Encoding still takes CPU time, and the preset does not promise an optimum or invisible loss. The published alpha.1 lacks this new command; [check development availability and exact policies](docs/AGENT_INTERFACE.md#automatic-folder-workflow) before use.

### Installer acquisition for agents

Use only this project's [Releases](https://github.com/terzima/media-compression/releases). Check macOS 14+ or Windows 11 x64 and select the native architecture. Prefer a stable release when available; the setup request also authorizes an unsigned public alpha preview. Use the explicit [first preview page](https://github.com/terzima/media-compression/releases/tag/v0.1.0-alpha.1) or list releases through the GitHub API and inspect `prerelease`, `draft`, assets and tag. GitHub's `/releases/latest` endpoint excludes prereleases; an absent latest stable release does not mean no preview exists. Match the installer to the platform entries in release-manifest.json.

| Computer | Public alpha.1 installer |
|---|---|
| Apple Silicon Mac | `Media-Compression_0.1.0-alpha.1_macos-arm64.dmg` |
| Intel Mac | `Media-Compression_0.1.0-alpha.1_macos-x64.dmg` |
| Windows x64 | `Media-Compression_0.1.0-alpha.1_windows-x64.exe` |

Download the matching installer, SHA256SUMS and release-manifest.json through their public release-asset links. Confirm the installer SHA-256/size match both records and identify the exact release tag/build commit. Checksums prove consistency with the release records, not signing or clean-device qualification. No GitHub account is needed to download public release assets. If the agent lacks download access, identify the exact asset the user must supply with its checksums; do not report installation success or trigger an expensive source build.

For internal testing only, Actions artifacts remain available with expiring downloads that may require GitHub access. [PR #2](https://github.com/terzima/media-compression/pull/2) records the original agent extension's successful native run. These artifacts use `candidate-<target>` names and contain installer checksums relative to `release/bundle/`, plus source/notices and build evidence. Public setup should use release assets instead.

On macOS, verify/mount the DMG read-only, copy the entire app bundle to `~/Applications`, and unmount it. On Windows, run the per-user NSIS installer with its bundled offline WebView2. Preserve an existing installation; if an update requires closing the running GUI, retain unsaved work and explain that step before replacement. If an OS dialog requires interaction, report it accurately and let the user complete it; do not remove quarantine or change system security policy. Verify the installed helper's `--version` and discovery commands. Report the release tag or exact development commit and installer hash. Downloading, checksum validation, installation, helper execution and MCP discovery are separate outcomes; report only those actually verified.

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

## Publishing another preview

Run `node scripts/set-version.mjs 0.1.0-alpha.2`, make the changes, update release notes/setup download examples, run relevant checks, and commit the exact release source. Push an annotated tag with the matching version (`v0.1.0-alpha.2`). The native workflow rebuilds/tests all three targets, checks source/license material, validates installer/source hashes and installed-agent evidence, then stages a draft release. It publishes that draft as a prerelease only after every upload's hash/size is verified. A failed build or upload leaves no published partial release. Published versions are not overwritten; fixes use a new version. Stable tags do not trigger this unsigned-preview publication path. See [release procedure](docs/RELEASE_QUALIFICATION.md#public-alpha-previews).

## License

Project-authored code is MIT. Dependencies retain their licenses. In particular pngquant/libimagequant are GPL components distributed as a separate executable with corresponding source; FFmpeg/LAME retain LGPL obligations. Each qualified release must include notices, matching source/build material, and checksums.
