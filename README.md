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

## Development

Install Node 24.21.0 and Rust 1.99.0. macOS needs Xcode command-line tools; Windows needs Visual Studio C++ tools, MSYS2 MinGW GCC/binutils/make. Install CMake 4.4.4 and Ninja 1.13.2 (for example in a build-only Python venv). Python is never bundled.

```sh
npm ci
npm run codecs
npm run worker
npm run check
cargo test --locked --workspace
cargo test --locked -p media-engine -- --include-ignored
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
