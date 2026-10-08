# Implementation plan

Approved by the owner on 2026-10-07. Implementation is authorized. Project brief and AGENTS.md govern the work.

## Product contract

Local desktop GUI on macOS 14+ (arm64 and Intel) and Windows 11 x64. No accounts, media uploads, API keys, separately installed codecs, or developer tools for users. Originals remain unchanged. Presets and custom settings share one engine. Explore runs actual candidate encodes on the user's files; results are measured, not estimates. Users may export any valid candidate regardless of diagnostic score or size savings.

Inputs: static PNG/JPEG/WebP; mono/stereo WAV/FLAC/MP3/AAC/M4A/Ogg Opus/Vorbis. Outputs: PNG/JPEG/WebP and AAC-LC/M4A/MP3/Opus/FLAC. Preserve dimensions/channels; explain sample-rate conversions. Apply orientation and supported color profiles for lossy images. Lossless images retain rendering-critical metadata. No silent precision loss. No trimming/normalization/downmixing. Remove private image metadata where safe; preserve basic audio tags and omit cover art.

Quick defaults: image quality 90/80/65; PNG starts lossless. AAC stereo 192/128/96 kbps, mono 96/64/48; MP3 VBR 2/4/6; Opus stereo 160/96/64, mono 64/48/32; FLAC level 5. Advanced controls expose supported quality, bitrate/VBR, effort, formats, and custom sweeps without a perceptual floor. Every candidate comes from the original; duplicate outputs merge into one candidate with all producing settings. Larger outputs are excluded from default selection but can be selected manually.

Image comparison: split, linked pan/zoom, fit/100%, checkerboard/light/dark; measured SSIM and alpha errors, decoded equality for lossless. Audio comparison: same-position switching, seek/loop/shared volume via native playback and bounded decoded PCM. Scores are diagnostics, never equivalence or inaudibility guarantees. Reports use hashes, relative names, settings/tool versions, actual bytes, properties, errors, and diagnostics.

Rust manages opaque IDs, queue states, progress/cancellation, verified cache, previews, playback and staged no-overwrite export. At most two workers; bound memory/threads, check disk, isolate image work in a process. Cancel process trees, clean unfinished work, retain completed candidates, continue partial batches. No arbitrary shell or network API. No Python runtime.

## Milestones

| Milestone | Status | Acceptance |
|---|---|---|
| 1. Reuse/dependency audit | Development audit complete; distribution review gated | Provenance, pins/licenses, public/generated fixtures |
| 2. Packaging proof | All three installed-bundle CI checks pass; device playback qualification remains open | Installed Mac arm64/Intel and Windows GUI finds bundled helpers, compresses image/audio, plays comparisons |
| 3. Engine/custom settings | Implemented; native integration checks pass locally | Presets/custom sweeps, validation, cancellation, cache and export integration tests |
| 4. Everyday GUI | Implemented; GUI tests and ARM64 launch pass | Files/folders/drop, accessible controls, responsive queue, per-file outcomes |
| 5. Comparison/report | Implemented; generated fixture checks pass | Actual output previews, measured color/alpha/timeline checks, JSON reports |
| 6. Release qualification | Deferred external infrastructure; remaining device gates open | Clean minimum-OS install, offline runtime, upgrade/uninstall, signing checks |
| 7. Publication | Public alpha preview authorized; stable publication gated | Three-target unsigned prerelease, checksums, matching source/notices and evidence; signed stable installers after qualification |

## Verification and release gates

Test codec endpoints/custom validation, hashes/cache/source changes, lossless equality, alpha/ICC/orientation/precision, short/long/silent mono/stereo audio, boundary transients, malformed/truncated files, Unicode/duplicates/symlinks, permissions/disk errors, interrupted export, crashes/cancellation and 1,000-file batches. Build/run native checks on Mac ARM, Intel and Windows; GUI test instrumentation stays out of public bundles.

Separate architecture-specific Mac DMGs and Windows per-user NSIS with offline WebView2. Sign Mac app/helpers with Developer ID, notarize/staple/test quarantine. Sign/timestamp Windows executables/installer (Microsoft Artifact Signing preferred); signing does not guarantee SmartScreen reputation. Hosted Windows CI has tools and disabled UAC, so clean Windows 11 install/runtime checks are still required. No unsigned stable release or claim of installation without checked evidence.

External inputs: signing identity/credentials and access to a clean Windows 11 test environment. Continue independent work while unavailable. No auto-updater/store distribution in v1. Deferred: resizing, surround, blind statistics, automatic quality targeting, ViSQOL, custom MP4 compaction, video and AI.

## Evidence and deviations

### Agent interface extension (authorized 2026-10-07)

Public alpha extension (authorized 2026-10-07): publish v0.1.0-alpha.1 with all three native installers and permanent public agent-setup downloads. Add synchronized version tooling, tag-triggered native rebuild/test/publication, integrity/provenance checks, complete source/license-text material and honest preview release notes. Stable signing/minimum-OS/distribution gates remain open. Keep failed uploads in a draft and never overwrite published versions.

External agent testing exercised generated image sweeps and FLAC studies with unchanged originals; its saved private evidence remains outside this repository/release. Feedback clarified terminal job-state documentation in the bundled guide (`ready`/`failed`/`canceled`, numeric `completed`, session `active`). Follow-up usability work: distinguish unsupported still-image input from video in probe errors, and expose explicit decoded-audio-equality diagnostics. Downloads permission/attachment materialization failures reported by that agent occurred before codec execution and do not establish a codec failure.

Deliver an optional native MCP stdio server and JSON CLI using the existing Rust engine and bundled codecs. Include discoverable tool schemas and a portable compression-study skill. No model/account/API key belongs in this application. Agents choose formats/settings, run real studies, refine candidates, and export explicitly selected results; diagnostic scores do not establish perceptual equivalence.

Acceptance: capabilities/import/study/status/results/preview/cancel/export tools; terminal inspect/study/export with persistent study manifests; bounded folder access, unchanged originals, collision-safe exports and verified cache reuse; separate locked agent workspace that leaves GUI sessions intact; native packaging and installed-agent checks on all three targets. Provide connection configuration from the GUI and document client setup. Signing/clean-machine publication gates remain deferred. Implementation and local verification are complete, including ARM64 installed-agent checks. The native matrix publishes per-commit installed-agent evidence; exact run outcomes are maintained on [the extension PR](https://github.com/terzima/media-compression/pull/2) and its artifacts.

- Local extension checks: MCP discovery, real image/audio studies, preflight, inline preview, explicit export, cancellation and active-client-disconnect shutdown; CLI saved-study deduplication, aggressive quality, no-overwrite collisions and changed-source rejection; scoped paths and workspace ownership; 12 frontend tests, Rust workspace Clippy and all seven existing bundled-codec integration tests (including the 1,000-file batch). The portable study skill validates. Native installers include the agent executable and skill; each target runs installed-agent checks using bundled helper discovery.
- ARM64 DMG at fd53e5e built from a clean tree and passed native dependency inspection and all four installed-agent interface tests. SHA-256: `b7a0f38774c11e5640a5c9f822d5f9998244969d08fe126b6f1682f0fc150268`. An isolated native QA app verified folder picking, bundled agent/skill paths, clipboard configuration, and Escape/focus restoration after picker return; the regression is covered by the GUI test. The final development build record and installer checksums supersede this initial artifact when rebuilt.
- Final local ARM64 app at 7944c65 includes the keyboard fix; clean-tree DMG/native inspection/installed-agent checks pass. SHA-256: `832a67d9239d362608ef96e92dda5f1cb013e399b8c878775ab76a5363466db7`. README now provides a copy-and-paste agent setup request and concrete bootstrap steps. A fifth interface test verifies guide/capabilities/tool/config discovery as valid JSON without creating a workspace.
- Agent setup handoff now covers machines without a download or installation: official release/development artifact acquisition, native architecture selection, checksums/build records, per-user installation and verified MCP/CLI discovery. All three targets passed run [37700839336](https://github.com/terzima/media-compression/actions/runs/37700839336) at b55da8a. No release is published; artifact access/expiry, OS interaction, signing and clean-machine qualification remain explicit limits. Documentation checks do not establish a fresh-machine installation test.

- 2026-10-07: clean standalone repository confirmed; authorized implementation starts on `codex/desktop-v1`.
- Upstream pins are resolved in committed lockfiles and SHA-256 source records. Glyph algorithms were reviewed and adapted as standalone code; no private media or app integrations were copied. See PROVENANCE.md and DEPENDENCIES.md.
- Implemented presets, advanced endpoint controls, custom sweeps, two-worker jobs, immediate queued cancellation, verified cache/deduplication, lazy color-managed previews, native audio comparisons, safe export and schema-versioned reports.
- Local pinned-toolchain checks: 11 frontend tests, Rust validation/process/color/PCM unit tests, and 7 real bundled-codec integration tests pass. The 1,000-file mixed batch completed in the integration suite (56.18 seconds total suite), with queued cancellation under five seconds and unchanged originals. Recovery checks verify re-import after repair/source changes, unreadable-folder partial imports and continuation of independent exports. Long mono audio, 8 kHz MP3 endpoints, Opus 0.5/256 kbps and Ogg stream tags are covered.
- ARM64 development DMG builds/mounts and its application bundle launches; a DMG copy was placed in ~/Applications. The DMG-copy helpers passed real image/audio encoding/export/PCM checks. Native audio device switching/pause/loop/shared-volume checks pass on this Mac. Loop bounds clamp to candidate duration, and stale playback preparation, stopped controls and late GUI subscriptions are covered. This is not a clean-machine/minimum-OS result.
- Native run [37686691317](https://github.com/terzima/media-compression/actions/runs/37686691317) at c5911c2 passed all three targets: frontend/native/real-codec tests (including JPEG export from a Unicode workspace), Clippy, matching source/notices, DMG/NSIS packaging, native dependency inspection, installed-copy image/audio studies and desktop launch smoke. Mac hosts run macOS 15; Windows runs Server 2025. This is development/CI evidence, not clean minimum-OS or interactive playback qualification. Earlier failed/canceled runs do not establish installation qualification.
- Windows embedded WebView2 input: SHA-256 `ac22ecdc19c5b88b87f3fa752c00da9541653a8f5c0c5fc4a3b2b6ebe6591f69`, file/product version `1.3.275.13`. Its Authenticode query returned no status/signer; signature qualification remains open and subsequent evidence preserves inspection errors explicitly.
- Final local ARM64 development DMG at 6676460 passed installed-helper studies and native dependency inspection. SHA-256: `085cc38615558c50868550818e6314bdb68929baf3189988f8697256c6eb1d93`. The comparison halves independently composite transparency against the selected background. Build/checksum/installed-copy records are generated under ignored artifacts/; CI uploads equivalent records with each installer.
- Owner decision: continue development builds and arrange signing/release infrastructure later. Stable publication remains gated on the platform/runtime/signing/distribution evidence in RELEASE_QUALIFICATION.md.
