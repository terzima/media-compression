# media-compression

An open-source desktop app for compressing images and audio, comparing quality against file size, and exporting the results.

## Status

Planning. There is no installable application or release yet.

The first release targets macOS and Windows. The goal is a straightforward download-and-install experience: users should not need Python, Node.js, Rust, Homebrew, or command-line codec setup.

## Intended workflow

1. Add files or folders.
2. Choose a sensible compression preset or explore multiple candidates.
3. Inspect size savings and compare images or listen to audio.
4. Export selected compressed files and an optional report, preserving originals.

Processing should happen locally without an account or uploading media.

See [the project brief](docs/PROJECT_BRIEF.md) for requirements and planning decisions. The desktop architecture, supported formats, and initial release scope are still to be finalized.

## License

Project-authored code is licensed under MIT. Third-party tools and models retain their own licenses; their distribution requirements must be documented before bundling them.
