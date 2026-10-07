# Release qualification

Current status: development candidate, not a stable release. Evidence applies to the exact recorded commit/artifact. A build does not establish installation, playback, minimum-OS compatibility, signing or reputation.

## Observed evidence (2026-10-07)

- Local Mac ARM64: frontend build/settings tests, Rust unit tests, bundled-codec integration tests, and Clippy pass. The 1,000-file mixed fixture batch completed accurately (56.49 seconds for the integration suite) and a second queued batch canceled within five seconds. Generated inputs cover transparency, lossy/lossless output, Unicode/collision export, cache tampering, changed sources, directory links, quoted report paths, repaired-input re-import, independent export continuation, PNG gamma, EXIF rotation, retained profiles, 16-bit PNG and 24-bit FLAC. Tiny images remain exportable without SSIM.
- A native ARM64 app bundle launched and showed working original/candidate comparison and compression results. The developer DMG builds, mounts with verified disk-image checksums, and copies to ~/Applications. A muted native device test passed timeline switching, pause, looping, shared volume and stop. The DMG-copy worker/helpers passed image/audio studies, safe export and playback PCM preparation; an isolated QA GUI launched and displayed actual image studies. This does not yet establish a generated-fixture installed GUI/audio end-to-end demonstration; this machine has a development environment.
- GitHub Actions run 37663238913: ARM64 native helper builds, engine/GUI compile checks, unit/integration tests, Clippy, source/notices packaging, and DMG packaging succeeded. Intel/Windows results are recorded below as they become available. Run 37668153907 passed ARM64 packaging as well; Intel exposed test readiness timing and Windows exposed response-file archiving. The fixes narrow FFmpeg to v1 functionality and add native dependency inspection/static Windows Rust runtimes. Windows archive paths/static zlib naming also required fixes; unsuccessful runs do not prove Windows support.
- Matching dependency source, vendor trees, a dependency manifest/notices, and a checksummed source tarball were generated locally. The packaged bundle contains native notices and dependency license resources. Final archive/source compliance audit remains a publication gate.

No clean Windows 11, minimum macOS 14, upgrade/uninstall, Gatekeeper quarantine or SmartScreen result has been claimed. No Developer ID Application identity is available on this machine. Current artifacts are unsigned/ad-hoc development builds, not notarized/stapled releases.

## Required gates

| Gate | Required evidence |
|---|---|
| Native builds | Mac arm64, Intel, Windows x64 helpers, tests, app and installer; exact commit and artifact SHA-256 |
| Installation | Clean macOS 14 on both architectures and Windows 11 with no codecs/dev tools; per-user install, launch, external output opening, upgrade and uninstall |
| Offline | Import, preset/custom study, image/audio comparison and export while disconnected; no required account/network |
| GUI | Files/folders/drop, keyboard operation, large-batch responsiveness, truthful partial outcomes; test instrumentation excluded from public builds |
| Reliability | Corrupted files, source changes, worker crashes, permissions, low disk/disk exhaustion, symlink/junction paths, canceled sweeps, interrupted exports; unchanged original hashes |
| Fidelity | Profiles/orientation/alpha/precision and codec boundaries, short/long/silent mono/stereo clips; confirm warnings and diagnostic limitations |
| Mac signing | Developer ID signs executable helpers/app, hardened runtime, successful notarization, stapled app/DMG; quarantine/Gatekeeper behavior on each architecture |
| Windows signing | Authenticode app/helpers/NSIS with trusted timestamp; observe SmartScreen behavior without promising absence of warnings |
| Distribution | Exact notices/licenses/source/build material, codec patent/distribution review, installer/source checksums and reproducible release notes |

## Publication procedure

Use the reviewed commit only. Rebuild all three native targets from pinned inputs, prepare source/notices before bundling, sign/notarize/staple where applicable, then repeat installation/runtime checks on the final signed artifacts. Record versions, artifact hashes, OS/device, steps, results, known limits and observed security prompts. Publish installers and matching source/notices/checksums together through GitHub Releases only when every stable gate passes. No automatic updater or app-store distribution in v1.

Owner decision (2026-10-07): continue development builds; arrange release infrastructure later.

Deferred owner inputs: Developer ID/notarization credentials, Windows signing identity/service and a clean Windows 11 installation-test environment. These do not block engine/GUI/CI development; they gate stable publication. Keep secrets outside repository/docs/reports.
