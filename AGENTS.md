# media-compression — working instructions

Read `docs/PROJECT_BRIEF.md` first. This is a standalone project; Glyph's learning mechanics, content rules, release flags, and application architecture do not apply here.

## Initial handoff

The user requested a new planning chat before implementation. Start with research and a concrete implementation plan, recommend defaults, and surface only consequential unresolved choices. Do not begin application implementation until the user approves the plan or explicitly asks to proceed. If native Plan mode is unavailable, maintain the same planning-only boundary and state that limitation accurately.

The user expects very little involvement once the plan is accepted. After approval, make routine decisions autonomously, keep a durable milestone plan, and continue through implementation, verification, and packaging.

## Product constraints

- macOS and Windows are first-release requirements. Do not ship a Mac-only implementation and describe it as cross-platform.
- End users must be able to install and use the GUI without a development environment or separately installed command-line codecs.
- Keep media processing local by default. No accounts, cloud uploads, API keys, or AI generation are needed for the initial compression workflow.
- Preserve originals by default. Write outputs separately, prevent accidental overwrite, and handle cancellation, corrupted inputs, large batches, Unicode paths, and partial failures honestly.
- Quality scores are diagnostics, not guarantees of perceptual equivalence or percentages of people who cannot notice differences. No universal inaudibility claims.
- Prefer established codecs and quality metrics. Reuse relevant algorithms and comparison UI where suitable; do not bring Glyph's private media, curriculum, credentials, analytics, or app-release integrations into this repository.
- Choose redistributable dependencies and document their exact versions, provenance, licenses, and required notices. An MIT license on this project does not relicense dependencies.
- Distinguish builds from verified installation/runtime behavior. Mac execution does not prove Windows compatibility. Use Windows CI and report any remaining real-device or signing limitations.
- Keep commits focused. Add tests for substantive behavior and run relevant checks. Do not claim a release is installable until it is built and its installation path is checked.

## Planning references

An ignored `LOCAL_CONTEXT.md` may provide paths to earlier experiments on the owner's machine. Those are reference material, not portable dependencies. Public documentation and released binaries must stand alone.
