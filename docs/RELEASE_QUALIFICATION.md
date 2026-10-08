# Release qualification

Current status: development candidate, not a stable release. Evidence applies to the exact recorded commit/artifact. A build does not establish installation, playback, minimum-OS compatibility, signing or reputation.

## Public alpha previews

The owner authorized publishing an unsigned `v0.1.0-alpha.1` testing preview on 2026-10-07 and a repeatable publication workflow. This explicitly permits public prerelease installers while signing and clean-device stable qualification remain deferred. It does not relax source/license packaging obligations or permit stable-release claims.

Alpha publication is triggered only by matching `vX.Y.Z-alpha.N` tags. All three native builds/tests must pass. Before uploading, the release assembler verifies exact commit/target, clean build records, synchronized manifest versions, installer/source hashes, native dependency and installed helper/agent/desktop evidence, and distribution material. Each installer is accompanied by per-target source/notices/evidence, the tagged project source, release-manifest.json and SHA256SUMS. Draft publication occurs only after every asset is uploaded and its server-reported digest/size matches; existing published versions are never replaced. If a run fails, correct the problem and rerun an unpublished draft only when its existing material still matches, or use a new version.

Release notes must state the unsigned/ad-hoc status, incomplete minimum-OS/interactive device and upgrade/uninstall qualification, observed WebView2 signature limitations and absence of an automatic updater. Public downloads need no GitHub account; an OS may still require user interaction or block unsigned code. The workflow does not disable OS protections or infer runtime success from a successful download. Stable signing/platform/distribution gates below remain required.

## Observed evidence (2026-10-07)

- Agent extension: local real-codec MCP/CLI studies, previews, cancellation/disconnect shutdown, saved-study/source verification, Unicode/folder exports and workspace scoping pass (five interface tests, including stateless setup discovery); 12 frontend tests and workspace Clippy pass. An ARM64 development DMG at fd53e5e includes the agent/skill and passes installed-agent checks with helper overrides disabled. Native folder selection, displayed bundled paths, clipboard copy and keyboard dismissal/focus restoration were checked in an isolated QA app. The final native matrix runs installed-agent tests on each target and records exact commit/artifact outcomes with [PR #2](https://github.com/terzima/media-compression/pull/2). This does not establish configuration in every third-party MCP client or clean-machine/signing qualification.

- Local Mac ARM64: frontend build/settings tests, Rust unit tests, bundled-codec integration tests, and Clippy pass. The 1,000-file mixed fixture batch completed accurately (56.18 seconds for the integration suite) and a second queued batch canceled within five seconds. Generated inputs cover transparency, lossy/lossless output, Unicode/collision export, cache tampering, changed sources, directory links, quoted report paths, repaired-input re-import, independent export continuation, PNG gamma, EXIF rotation, retained profiles, 16-bit PNG and 24-bit FLAC. Tiny images remain exportable without SSIM.
- A native ARM64 app bundle launched and showed working original/candidate comparison and compression results. The developer DMG builds, mounts with verified disk-image checksums, and copies to ~/Applications. A muted native device test passed timeline switching, pause, looping, shared volume and stop. Unit checks cover loop bounds longer than a candidate and out-of-range starts; 11 frontend tests cover custom settings, explicit larger-output export, error reporting, progress, audio resets and late event cleanup. The DMG-copy worker/helpers passed image/audio studies, safe export and playback PCM preparation; an isolated QA GUI launched and displayed actual image studies. This does not yet establish a generated-fixture installed GUI/audio end-to-end demonstration; this machine has a development environment.
- Native run [37686691317](https://github.com/terzima/media-compression/actions/runs/37686691317) at c5911c2 passed all three targets: frontend/native/real-codec tests including Unicode JPEG paths, Clippy, matching source/notices, DMG/NSIS packaging, dependency inspection, installed-copy image/audio studies and five-second desktop launch smoke. Mac hosts run macOS 15; the Windows runner is Windows Server 2025, not a clean Windows 11 machine. CI PCM preparation and launch smoke do not establish interactive playback on Intel/Windows. Unsuccessful/canceled jobs do not establish installation/runtime qualification.
- Windows embedded offline WebView2 input from the recorded Microsoft distribution URL has file/product version `1.3.275.13` and SHA-256 `ac22ecdc19c5b88b87f3fa752c00da9541653a8f5c0c5fc4a3b2b6ebe6591f69`. Its Authenticode inspection returned no status/signer; it is unqualified, not confirmed valid or invalid. Inspection errors are preserved in subsequent provenance records. Publisher/signature assessment remains required before stable distribution.
- The final local ARM64 DMG at 6676460 has SHA-256 `085cc38615558c50868550818e6314bdb68929baf3189988f8697256c6eb1d93`; native dependency inspection and installed-copy codec studies pass. Its JSON build record confirms a clean source tree at build time. The app is copied to ~/Applications; existing comparison sessions were left running. Quit an earlier public preview before launching the updated copy because the backend locks its workspace against concurrent instances.
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
