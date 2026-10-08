# Agent connection and terminal interface

The optional native `media-compression-agent` executable provides MCP stdio tools and a JSON CLI using the same engine/codecs as the GUI. It contains no model, account, API key, network listener, Python or Node runtime. A connected agent supplies its own reasoning and its own model/data policies. Image previews and structured results returned to that agent are governed by those policies.

## Connect an MCP client

Users can paste the setup request from [the README](../README.md#set-it-up-with-your-agent) into a terminal-capable agent, even before downloading the app. The README covers public preview installer acquisition, architecture/checksum/build verification, per-user installation, executable discovery, guide/schema loading, folder grants, client configuration and connection verification. Public release assets need no GitHub account; an unsigned build may need native OS interaction. Absent installers are reported honestly. The CLI is the fallback when client setup is unavailable.

In the GUI, choose **Agent connection → Choose granted folder**. Copy the displayed configuration into your client's MCP settings. The generated command is the actual bundled executable; its arguments grant only the chosen folder. The JSON `mcpServers` layout is accepted by compatible clients; clients with another configuration format can use the same command/arguments. Multiple grants use repeated `--root` arguments.

On macOS the installed executable is inside `Media Compression.app/Contents/MacOS/media-compression-agent`. On Windows it is `media-compression-agent.exe` beside the installed GUI executable. Helpers and the study skill are bundled in the same installer. No system codecs or developer environment are needed to run it.

The `agent-plugin/skills/media-compression/SKILL.md` skill is also embedded in server instructions. It describes actual-study refinement, metric limitations, explicit export selection, source preservation and partial failures. This is a portable MCP/skill bundle, not an already-installed client plugin or marketplace publication. Client setup is the only connection step; ordinary users can keep using the GUI without it.

Tools:

| Tool | Purpose |
|---|---|
| compression_capabilities | Supported settings, codec constraints, limits and granted folders |
| import_media / list_media | Inspect file/folder inputs, paginate IDs/properties/errors |
| plan_study | Validate proposed settings and disclose encode count/rate conversions without processing |
| start_study / job_status | Start bounded background work and read progress/outcomes |
| get_candidates | Per-file exact bytes/savings, hashes, settings, properties and diagnostics |
| preview_image | Color-managed image thumbnail for an original/candidate; output dimensions stay unchanged |
| cancel_study | Cancel queued/active jobs; retain completed candidates |
| export_candidates | Verified, staged, no-overwrite export of explicitly chosen IDs; optional sanitized report |
| save_study | Save completed/canceled results for later CLI export without overwriting |

Lossless selection can use verified decoded equality. Lossy selection needs the user's requirements and appropriate visual/listening feedback. Scores do not establish perceptual equivalence, percentages of people who cannot notice differences, or a universal optimum. Candidates are always encoded from originals; aggressive/larger outputs remain selectable.

For asynchronous studies, track the IDs returned by start_study. Terminal job states are `ready` (one or more valid candidates), `failed` and `canceled`. `completed` is a numeric count of attempted settings, not a state. Inspect each job's errors and actual candidates even when its state is `ready`; partial failures are retained. Top-level `active=false` indicates no queued/running jobs remain in the session. Never poll for a nonexistent `completed` state.

## Terminal fallback

Invoke the installed executable by its full path or an alias configured by the agent. The examples use `media-compression-agent` for readability; installation does not modify PATH or shell profiles.

```sh
media-compression-agent --help
media-compression-agent capabilities
media-compression-agent tools
media-compression-agent guide
media-compression-agent config --root /media
media-compression-agent inspect /media/image.png --root /media
media-compression-agent study --input /media/request.json --manifest /media/study.json --root /media
media-compression-agent export --input /media/export-request.json --root /media
```

Study request (quality controls are encoder settings):

```json
{
  "paths": ["/media/image.png"],
  "settings": [
    {"format": "webp", "lossless": false, "quality": 90},
    {"format": "webp", "lossless": false, "quality": 80},
    {"format": "webp", "lossless": false, "quality": 60},
    {"format": "webp", "lossless": true, "effort": 6}
  ]
}
```

The CLI waits for study completion and returns JSON with opaque source/candidate IDs, exact settings/bytes/savings, diagnostics, per-file/job errors and the manifest location. Refine with another request and a new manifest filename. The verified cache avoids repeating identical encodes. A single CLI study uses one settings list across its inputs; separate image/audio requests or MCP per-file settings handle mixed workflows.

Export request:

```json
{
  "study": "/media/study.json",
  "candidateIds": ["candidate ID from the result"],
  "destination": "/media/export",
  "report": true
}
```

Later export imports only the selected source files, restores their folder-relative names, verifies original hashes, and regenerates/verifies selected candidates using current bundled codecs and verified caches. Unchanged originals are required. Missing/changed sources or candidate hash differences are reported; unaffected selected files can still export. Manifests contain private local paths and are not sanitized public reports. A manually edited manifest cannot direct the engine to copy an arbitrary candidate path.

Data commands return versioned JSON on stdout; failures return JSON on stderr. MCP reserves stdout for protocol messages. Exit codes: 0 success, 1 invalid request/runtime failure, 2 partial file failure, 130 interrupted. `--input -` reads JSON from stdin. Input/manifest JSON is limited to 64 MiB; encoder settings use the engine's bounds. Ctrl-C cancels work; completed study candidates can be saved for later export.

MCP requires explicit `--root` grants. The CLI defaults to its current directory; use `--root` for inputs/exports elsewhere. Canonical paths and discovered files are checked before probing, and output ancestors are checked before writing. This is application-level folder scoping, not an OS sandbox against concurrent hostile filesystem changes.

Agent and GUI workspaces are separate. The default agent cache is the platform cache directory under `media-compression/agent-v1`. A workspace lock permits one agent process at a time; use `--workspace` with a distinct empty/owned directory for another client. Keep custom cache workspaces outside imported folders. Startup rejects nonempty unowned workspaces. MCP disconnect cancels jobs and shuts down helper processes. Each engine uses at most two workers. Agent studies do not automatically appear in an already-open GUI session; exported files open normally, and the GUI remains independently available.
