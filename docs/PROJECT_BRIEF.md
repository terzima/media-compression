# Project brief

## User objective

Make a useful open-source application that other people can easily download, install, and use for their own media compression needs. Repository and app working name: `media-compression`. First-release platforms: macOS and Windows.

The owner accepts most reasonable recommendations and expects to provide very little guidance during the build. Begin in planning mode to settle final details; do not treat that as authorization to skip planning and immediately build the app.

## Product direction

One desktop app with Image and Audio workflows. The distinctive workflow is generating compression candidates, measuring size and quality, comparing the candidates, and choosing what to export. Offer simple defaults for people who only want smaller files, with advanced testing available without making it mandatory.

Initial proposed capabilities:

- Add individual files, folders, or batches through drag-and-drop and a file picker.
- Quick compression presets and an optional sweep across several settings.
- Original/candidate byte counts and savings, clearly separated from quality metrics.
- Image comparison with transparency-aware previews and useful zoom controls.
- Audio comparison playback, optionally with blind comparison in an advanced workflow.
- Progress, cancellation, per-file errors, and explicit results for unsupported inputs.
- Export selected variants and an optional machine-readable report without changing originals.
- Local processing with no account, network service, or developer-tool installation required.

These are planning inputs, not a mandate to overbuild every advanced feature in v1. Recommend a coherent first release and identify later additions. Video compression and generative image/audio services are not established first-release requirements.

## Architecture recommendation to evaluate

Tauri with a web GUI and bundled compression helpers is the initial recommendation. Existing research used Python orchestration, browser-based comparisons, pngquant/oxipng/DSSIM for PNGs, and Apple's AAC encoder plus ViSQOL for audio.

Evaluate whether maintaining packaged Python helpers is worthwhile versus a smaller Rust/helper implementation. Prefer the simplest maintainable route that actually meets both platform and installer requirements; do not rewrite algorithms just for aesthetic consistency.

The earlier audio encoder is macOS-specific. A cross-platform audio backend (for example a carefully selected FFmpeg build) must be chosen, bundled legally, and tested. Previous Apple AAC quality-versus-size results cannot be presented as validation of another encoder. ViSQOL build/model packaging and support for short clips require particular attention; a useful basic compression app should not depend on a fragile research-tool setup.

## Decisions the initial plan must resolve

1. GUI/backend stack and helper packaging, with a reasoned default.
2. Initial input/output formats and what happens to metadata, color profiles, alpha, audio channels, sample rate, and duration.
3. Presets versus automatic quality-target selection; metric limits and inspection workflows.
4. macOS architecture support and initial Windows architecture support.
5. Dependency versions, reproducible acquisition/builds, licensing, models, and notices.
6. GitHub Actions build/test matrix, installer formats, signing/notarization path, and GitHub Releases.
7. A phased implementation plan with acceptance checks and a short first-run demonstration.
8. Any genuinely blocking owner inputs, such as signing credentials; continue independent work while those are unavailable.

## Acceptance target

A person can download the appropriate installer from GitHub Releases, install and launch the application, add their own image/audio files, compress them, inspect the results, and export valid outputs without opening a terminal or installing dependencies separately.

The released app preserves original files by default, handles errors and cancellation without leaving misleading success states, documents supported formats and limitations, includes required third-party notices, and has verified Mac and Windows build/runtime evidence appropriate to the release claims.

## Prior evidence

Earlier experiments were conducted while reducing an iOS app's media footprint. They provide candidate-generation, integrity, caching, report, and comparison ideas. Their measured savings are corpus-specific and their release-specific code is not a generic engine. Reuse selectively after reviewing each source file; do not copy the parent app or its media library.
