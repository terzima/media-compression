---
name: media-compression
description: Use the local Media Compression tools or JSON CLI to study image/audio compression, compare actual size and quality diagnostics, refine settings, and export chosen candidates without changing originals.
---

Use the MCP tools when available. For terminal-only access, run the installed media-compression-agent executable with --help; capabilities and tools return JSON. The application bundles its codecs; do not install replacement encoders or call external media-upload/encoding services as part of this workflow.

Check compression_capabilities and imported properties before choosing settings. Preserve dimensions/channels and explain any required sample-rate conversion. Transparency-to-JPEG requires an explicit background. Keep access within the configured granted folders.

Run real studies from the originals. Begin with a useful spread of supported settings/formats, use plan_study to check work/conversions, inspect exact bytes and diagnostics, and refine around promising candidates. Size/quality can be non-monotonic, so do not assume binary search finds a global optimum. Deduplicated candidates retain all producing settings. Poll job_status with reasonable intervals. Successful jobs end in state `ready`; other terminal states are `failed` and `canceled`. There is no `completed` state: `completed` is the count of attempted settings. Track the returned job IDs and inspect errors even for `ready` jobs; partial failures may coexist with valid candidates. Top-level `active=false` means no queued/running jobs remain in the session. Completed candidates remain available after failure/cancellation.

For lossless requests, require verified decoded equality and choose the smallest tested valid candidate. For lossy requests, use the user's requirements, diagnostics and visual/listening feedback to identify acceptable tradeoffs. Image quality is an encoder control, not a similarity percentage. SSIM/alpha/timing do not prove perceptual equivalence; audio needs listening where perceived quality matters. Do not claim a globally optimal or universally inaudible result.

Use get_candidates per file and preview_image where useful. Present exact settings/bytes/savings and relevant notices for the recommended candidate and meaningful alternatives. Aggressive or larger outputs may be deliberately selected; do not impose a universal quality floor. Export only the candidates covered by the user's instruction and verify each outcome; collisions are numbered and originals are preserved. Do not repeatedly ask for approval already given for a specified export.

Save completed studies with save_study when the user wants later export or reproducibility. Study manifests contain private local paths; share the separate sanitized JSON export report when requested. CLI export reopens a saved study, verifies source hashes and regenerates/verifies selected outputs with current bundled codecs. Changed/missing sources and failed candidates must remain visible as errors.

The GUI works independently for human comparisons. Agent and GUI workspaces are separate; agent studies do not automatically populate an already-open GUI session. Exported candidates can be opened normally. No AI model, account or API key is required by this application; the connected agent supplies its own reasoning.
