# The agent's compression recipe book

The app bundles a versioned recipe catalog with concrete settings and MCP/CLI execution steps for images **and audio**. Your agent reads a recipe, fills in your files and destination, checks actual media properties, runs the commands and returns the outputs. You provide the goal; the agent handles parameters and command syntax.

For example:

```text
Use Media Compression's recipe book on [files or folder]. I want
[exact preservation / smaller copies / a measured study]. Save the results
in [output folder] and give me the images and audio. Ask only for missing
essential information, preserve originals, and complete the exports.
```

Keep using that goal/destination for later requests when appropriate. A recipe is reusable; it does not mean every input can produce a smaller output or that a lossy setting is invisible/inaudible.

| Recipe | What the agent runs |
|---|---|
| `image-exact` | Supported PNG/WebP lossless alternatives, verified decoded equality, smallest tested smaller copy |
| `image-smaller` | One WebP quality 90 preset with transparency retained |
| `image-measured-study` | Lossless baseline and WebP 98/95/90, real bytes/SSIM/alpha errors, useful refinement |
| `image-compatible-format` | Requested PNG/JPEG/WebP output; explicit JPEG background when transparent |
| `audio-exact` | Supported 16-/24-bit integer sources to FLAC levels 5/8, verified sample equality |
| `audio-smaller` | AAC-LC/M4A 96 kbps mono or 192 kbps stereo, supported source rate retained |
| `audio-listening-study` | AAC bitrate, MP3 VBR or Opus bitrate alternatives, actual bytes/timing and listening feedback |
| `custom-study` | Your chosen supported image quality, audio bitrate/VBR and effort values |
| `size-budget-study` | Bounded refinement against your explicit byte budget and quality/format constraints |
| `folder-copies` | One automatic Lossless, Smaller or Both operation for a mixed image/audio folder |

No universal quality threshold is baked into the study recipes. They use real encodes; exact preservation requires decoded equality, while perceptual choices use diagnostics and inspection/listening. If you authorize the agent to recommend and export, it should deliver its recommendation with an honest qualification rather than stopping to ask again. Requests to review first remain review-first workflows. If a valid candidate is not smaller, simple compression recipes return the original with the reason. Custom/compatibility requests can deliberately export larger valid outputs.

## Discovery for agents

MCP: call `compression_recipes({})` to discover the catalog, or `compression_recipes({"recipeId":"audio-exact"})` for one recipe and its shared execution steps. It is read-only and does not import/process/export files. The server's bundled skill routes plain-language goals to these IDs.

Terminal: run the installed executable by its full path with `recipes`, or `recipes audio-exact`. The examples below assume the agent has resolved the executable; installation does not modify PATH.

```sh
media-compression-agent recipes
media-compression-agent recipes image-measured-study
media-compression-agent recipes audio-listening-study
```

The portable skill also contains [recipes.json](../agent-plugin/skills/media-compression/recipes.json). One catalog is embedded in the executable, served through both interfaces and copied into installers. `schemaVersion` describes the response structure; `recipeVersion` identifies this catalog's revision. Read only the relevant recipe once for a workflow; check installed capabilities instead of assuming every variant fits every source.

Each study recipe supplies `variants` with `conditions` and directly usable encoder `settings`. Match the requested format, channel count, precision and profile constraints; validate the settings against imported properties. Omit unsupported alternatives without weakening the user's goal. MCP `plan_study` checks work/settings/conversions before `start_study`; CLI `inspect`/`capabilities` provides properties and bounds before its blocking `study`. The shared workflow explains status polling, candidate selection and verified export. Custom/size-budget recipes intentionally accept user requirements rather than hard-coded acceptance thresholds.

Disclose any required sample-rate conversion before processing. Opus uses 48 kHz; there is no automatic trimming, normalization or channel mixing. Timing is a diagnostic, not an audio similarity score. A lossy source cannot regain lost information by conversion to FLAC. A thumbnail is not full-resolution visual verification. Video, resizing, surround and unsupported precision conversions are outside these recipes.

Recipe discovery is a development extension after `v0.1.0-alpha.1`, requiring an updated build. On older installations, the agent can still follow the existing guide and study/export tools; it must not claim an absent command/tool is available. See [setup](../README.md#set-it-up-with-your-agent), [interface schemas](AGENT_INTERFACE.md), and [the GUI walkthrough](USER_GUIDE.md).
