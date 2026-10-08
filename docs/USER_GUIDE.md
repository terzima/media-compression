# Ask for smaller files in plain language

Your existing agent can provide the conversation: ask for missing details, run Media Compression locally, and return the result. Use an agent client with local MCP or terminal access after [setup](../README.md#set-it-up-with-your-agent). The app supplies compression tools and instructions, not its own AI model or another subscription. A chat client without local tools cannot operate the app merely because you have an AI subscription. Your agent's handling of previews/results follows that client's data policies.

You only need to provide a local file or folder and where to save copies. An attachment works when your client can make it available locally. You do not need encoder names, terminal commands, quality numbers, or a proposed study. The agent should reuse choices you already made, ask only for consequential missing information, and complete the exports you authorized.

## Requests you can copy

**Help me decide, then do it:**

```text
Use Media Compression to make [image file] smaller and save a copy in
[output folder]. Help me choose between exactly unchanged pixels and a
visually similar smaller copy. Ask only what you need, then run the work
and give me the image. Preserve my original.
```

**Exactly unchanged pixels, without a visual review:**

```text
Find a smaller copy of [image file] with exactly the same decoded pixels.
Test supported lossless alternatives, choose the smallest tested valid
result, and save it in [output folder]. Preserve dimensions, transparency,
and my original. If nothing is smaller, tell me and keep the original.
Give me the image and its before/after sizes.
```

**A measured study with lossy compression allowed:**

```text
Run a measured compression study on [image file]. I allow lossy copies.
Recommend a much smaller, visually similar image, export it to [output
folder], and give me the image with its size savings and measured quality.
Preserve dimensions, transparency, and my original. Explain any remaining
uncertainty briefly; do not claim a score proves invisibility.
```

If you want to inspect before export, replace “export it” with “show me the useful alternatives before export.” You can also ask: “Create Lossless and Smaller copies of this folder.” The automatic folder command uses documented presets; a custom study explores additional settings.

## What a study actually does

Each setting encodes a fresh candidate from the original. The engine decodes outputs for verification and measures their bytes and diagnostics. Identical outputs are deduplicated while retaining every setting that produced them. Quick study tests preset levels for the selected format; Advanced tests your listed settings. A lossless study verifies one displayed configuration per run. Compare other supported lossless formats/efforts with additional runs. A study does not search every possible codec/setting or automatically find your personal invisible-loss boundary.

| Result | What it establishes |
|---|---|
| Exact output bytes and savings | Actual size reduction for that encoded file |
| Decoded pixels: Identical, with lossless settings | Verified unchanged decoded image content; file metadata/bytes can differ |
| SSIM on light/dark backgrounds | Structural similarity of color-managed composites; useful for comparing tested candidates |
| Alpha maximum/mean error | Measured transparency differences, separately from the composites |
| Encoder quality, such as 95 | A codec control, not 95% visual similarity |

SSIM does not establish that you personally cannot notice a difference. Viewing scale and distance affect its interpretation, as described in the [SSIM authors' guidance](https://www.cns.nyu.edu/~lcv/ssim/). There is no universal score in this app that means “invisible.” Use decoded equality for a verifiable unchanged-image requirement, or combine measured lossy results with inspection for a perceptual choice. The smallest tested acceptable candidate is not a proven global optimum. Exact preservation can produce much less savings than lossy compression; an already efficient original may win.

## Use the GUI yourself

For **exact pixels**:

1. Add your image and select it in the file list.
2. Choose **PNG** or **WebP** under Output format, then **Lossless** under Compression type. For the published alpha.1, choose **Advanced** as well; its Quick study otherwise includes lossy alternatives. The development update respects explicit Lossless in both modes.
3. Click **Quick study**. Lossless uses the displayed effort/configuration; the caption in the update says **Verify one lossless setting**. Try the other supported format or another effort with an additional run if useful. Unsupported precision/color combinations produce an explanation.
4. Click a candidate and check **Decoded pixels → Identical**, its size, exact settings and notices. A lossless claim also requires lossless settings; SSIM alone is insufficient.
5. Review candidate checkboxes, select your intended result, and use **Export selected** to choose an output folder. Clicking a candidate inspects it; its checkbox selects it for export. You can deliberately export a larger output, although it is excluded from default selection.

For **visually similar copies with more savings**:

1. Choose **WebP**, **Advanced**, and **Lossy · adjustable quality**.
2. Enter a small starting spread such as `98,95,90` in **Study quality settings**, then click **Quick study**. Those numbers are encoder settings, not scientific acceptance thresholds. Default Quick mode instead compares 90/80/65; PNG/WebP defaults also include a lossless baseline.
3. Click useful candidates to compare exact bytes, SSIM and alpha errors. Compare at **100%**, move the split slider, scroll to detailed areas, and try light/dark backgrounds for transparency. Fit view can hide artifacts.
4. Refine with additional levels if needed. Check the candidate you want and export it. JPEG removes transparency only with an explicitly chosen background; unsupported precision is not silently reduced.

Image dimensions stay unchanged. The GUI compares full-resolution previews; agent MCP previews are thumbnails. Agent studies and GUI studies use separate workspaces, so an agent's candidates do not automatically appear in an already-open GUI. Open its exported image normally.

For audio, Quick study compares actual bitrate/VBR presets, or one FLAC lossless setting. Switch Original/Candidate at the same timeline position and listen to important passages. Timing diagnostics do not establish inaudibility. FLAC lossless claims are limited to supported 16-/24-bit integer sources; lossy sources cannot regain discarded information by conversion to FLAC.

## Availability

The public `v0.1.0-alpha.1` already supports measured studies, comparison and export. The conversational guide update, inline **How to choose a result** help, explicit Quick-mode Lossless correction and automatic folder command are development changes after alpha.1. They require an updated build; editing a prompt does not add absent executable features. Agents should read the installed executable's `guide`, `capabilities` and `tools` and describe the interface actually available.
