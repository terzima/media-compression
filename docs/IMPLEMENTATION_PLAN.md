# Implementation plan

Approved by the owner on 2026-10-07. Implementation is authorized. Project brief and AGENTS.md govern the work.

## Product contract

Local desktop GUI on macOS 14+ (arm64 and Intel) and Windows 11 x64. No accounts, media uploads, API keys, separately installed codecs, or developer tools for users. Originals remain unchanged. Presets and custom settings share one engine. Explore runs actual candidate encodes on the user's files; results are measured, not estimates. Users may export any valid candidate regardless of diagnostic score or size savings.

Inputs: static PNG/JPEG/WebP; mono/stereo WAV/FLAC/MP3/AAC/M4A/Ogg Opus/Vorbis. Outputs: PNG/JPEG/WebP and AAC-LC/M4A/MP3/Opus/FLAC. Preserve dimensions/channels; explain sample-rate conversions. Apply orientation and supported color profiles for lossy images. Lossless images retain rendering-critical metadata. No silent precision loss. No trimming/normalization/downmixing. Remove private image metadata where safe; preserve basic audio tags and omit cover art.

Quick defaults: image quality 90/80/65; PNG starts lossless. AAC stereo 192/128/96 kbps, mono 96/64/48; MP3 VBR 2/4/6; Opus stereo 160/96/64, mono 64/48/32; FLAC level 5. Advanced controls expose supported quality, bitrate/VBR, effort, formats, and custom sweeps without a perceptual floor. Every candidate comes from the original; duplicate outputs share storage. Larger outputs are excluded from default selection but can be selected manually.

Image comparison: split, linked pan/zoom, fit/100%, checkerboard/light/dark; measured SSIM and alpha errors, decoded equality for lossless. Audio comparison: same-position switching, seek/loop/shared volume via native playback and bounded decoded PCM. Scores are diagnostics, never equivalence or inaudibility guarantees. Reports use hashes, relative names, settings/tool versions, actual bytes, properties, errors, and diagnostics.

Rust manages opaque IDs, queue states, progress/cancellation, verified cache, previews, playback and staged no-overwrite export. At most two workers; bound memory/threads, check disk, isolate image work in a process. Cancel process trees, clean unfinished work, retain completed candidates, continue partial batches. No arbitrary shell or network API. No Python runtime.

## Milestones

| Milestone | Status | Acceptance |
|---|---|---|
| 1. Reuse/dependency audit | Development audit complete; distribution review gated | Provenance, pins/licenses, public/generated fixtures |
| 2. Packaging proof | ARM64 development proof; Intel/Windows CI in progress | Installed Mac arm64/Intel and Windows GUI finds bundled helpers, compresses image/audio, plays comparisons |
| 3. Engine/custom settings | Implemented; native integration checks pass locally | Presets/custom sweeps, validation, cancellation, cache and export integration tests |
| 4. Everyday GUI | Implemented; GUI tests and ARM64 launch pass | Files/folders/drop, accessible controls, responsive queue, per-file outcomes |
| 5. Comparison/report | Implemented; generated fixture checks pass | Actual output previews, measured color/alpha/timeline checks, JSON reports |
| 6. Release qualification | Deferred external infrastructure; remaining device gates open | Clean minimum-OS install, offline runtime, upgrade/uninstall, signing checks |
| 7. Publication | Gated; development artifacts only | Qualified GitHub installers, checksums, dependency source/notices and evidence |

## Verification and release gates

Test codec endpoints/custom validation, hashes/cache/source changes, lossless equality, alpha/ICC/orientation/precision, short/long/silent mono/stereo audio, boundary transients, malformed/truncated files, Unicode/duplicates/symlinks, permissions/disk errors, interrupted export, crashes/cancellation and 1,000-file batches. Build/run native checks on Mac ARM, Intel and Windows; GUI test instrumentation stays out of public bundles.

Separate architecture-specific Mac DMGs and Windows per-user NSIS with offline WebView2. Sign Mac app/helpers with Developer ID, notarize/staple/test quarantine. Sign/timestamp Windows executables/installer (Microsoft Artifact Signing preferred); signing does not guarantee SmartScreen reputation. Hosted Windows CI has tools and disabled UAC, so clean Windows 11 install/runtime checks are still required. No unsigned stable release or claim of installation without checked evidence.

External inputs: signing identity/credentials and access to a clean Windows 11 test environment. Continue independent work while unavailable. No auto-updater/store distribution in v1. Deferred: resizing, surround, blind statistics, automatic quality targeting, ViSQOL, custom MP4 compaction, video and AI.

## Evidence and deviations

- 2026-10-07: clean standalone repository confirmed; authorized implementation starts on `codex/desktop-v1`.
- Upstream pins are resolved in committed lockfiles and SHA-256 source records. Glyph algorithms were reviewed and adapted as standalone code; no private media or app integrations were copied. See PROVENANCE.md and DEPENDENCIES.md.
- Implemented presets, advanced endpoint controls, custom sweeps, two-worker jobs, immediate queued cancellation, verified cache/deduplication, lazy color-managed previews, native audio comparisons, safe export and schema-versioned reports.
- Local pinned-toolchain checks: 9 frontend tests, Rust validation/process/color/PCM unit tests, and 7 real bundled-codec integration tests pass. The 1,000-file mixed batch completed in the integration suite (56.49 seconds total suite), with queued cancellation under five seconds and unchanged originals. Recovery checks verify re-import after repair/source changes, unreadable-folder partial imports and continuation of independent exports. Long mono audio, 8 kHz MP3 endpoints, Opus 0.5/256 kbps and Ogg stream tags are covered.
- ARM64 development DMG builds/mounts and an app copied to ~/Applications launches. The DMG-copy helpers passed real image/audio encoding/export/PCM checks. Native audio device switching/pause/loop/shared-volume checks pass on this Mac. This is not a clean-machine/minimum-OS result.
- Mac arm64 CI run 37663238913 passed native builds/tests and DMG/source packaging. Run 37668153907 also passed ARM64 tests/DMG packaging; Intel exposed a GUI test readiness race and Windows exposed FFmpeg response-file archiving. Fixes and final-target CI remain in progress; failed runs are not platform acceptance evidence.
- Owner decision: continue development builds and arrange signing/release infrastructure later. Stable publication remains gated on the platform/runtime/signing/distribution evidence in RELEASE_QUALIFICATION.md.
