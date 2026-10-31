# Sprint 001 contracts

This implements issue #1 on base `9952d9b`. The approved issue/sprint brief is
the design authority. Existing Tauri 2, React and packaging are retained. Rust
owns document bytes; React holds snapshots and transient pointer/playback state.
An independent core crate allows deterministic tests without a desktop runtime.

## Document and IPC

JSON fields use `snake_case`; Tauri top-level invoke arguments use `camelCase`.
`Rgba = [u8; 4]`, `Point = {x: u32, y: u32}`. IDs are opaque strings.
Dimensions are 16/32/64/128; initial document is 64x64, one layer, one 100ms frame.
Layers are bottom-to-top; each layer/frame has an independent RGBA cel. Adding a
frame copies the active frame, without linking mutable cels. Maximum 16 layers,
64 frames, 65536 stroke points; durations 20..10000ms. Revision is monotonic,
including undo/redo. Mutations validate the revision and whole request before
changing state. A pointer stroke is one transaction; core interpolates points.

`Snapshot` contains `document_id, revision, width, height, layers, frames,
active_layer_id, active_frame_id, palette, pixels, can_undo, can_redo, dirty`.
`layers` entries: `{id, name, visible}`; `frames`: `{id, duration_ms}`.
`pixels` is row-major composited RGBA bytes of the active frame. Selection does
not increment revision or create history. Palette is stored in Rust.

`EditCommand` is internally tagged by `type` (snake_case variants):

- `paint {layer_id, frame_id, points, color}`
- `fill {layer_id, frame_id, x, y, color}`
- `replace_cel {layer_id, frame_id, pixels}` (exact RGBA length)
- `add_layer {name}`, `set_layer_visibility {layer_id, visible}`
- `add_frame {}`, `set_duration {frame_id, duration_ms}`
- `select_layer {layer_id}`, `select_frame {frame_id}`
- `undo {}`, `redo {}`

Core API: `Editor::new(width,height) -> Result<Editor,String>`,
`Editor::snapshot() -> Snapshot`, `Editor::execute(expected_revision,command)
-> Result<Snapshot,String>`, `Editor::frame_pixels(frame_id) -> Result<Vec<u8>,String>`,
`Editor::cel_pixels(layer_id,frame_id) -> Result<Vec<u8>,String>`,
`Editor::save(path: &Path) -> Result<(),String>`,
`Editor::load(path: &Path) -> Result<Editor,String>`,
`Editor::export_sheet(path: &Path) -> Result<(),String>` (PNG + adjacent JSON),
`Editor::export_frame(path: &Path) -> Result<(),String>`.
Versioned `.spriteproj` ZIP contains manifest and lossless PNG cels. Bounded
decoding rejects corrupt/oversized entries and invalid IDs; saves replace
atomically, leaving original intact on failure. History is bounded.

Tauri commands: `document_snapshot`, `document_create {width,height}`,
`document_command {expectedRevision,command}`, `project_save {name}`,
`project_load {name}`, `project_export {name}`. The first five return Snapshot;
export returns an output-path string. A document may overwrite only its own
saved/loaded target; existing unrelated names are refused. Project names are single safe filenames;
storage is under app-data/projects, with no model-controlled filesystem paths.

## Easel proposals

Client crate is independent of Tauri. Credentials come from Rust environment,
never IPC or frontend state. `EASEL_URL` defaults to `https://easel.ait4x.org`;
`EASEL_API_KEY` is used only at that configured origin. HTTP allowed only for
literal loopback addresses; HTTPS otherwise. Redirects are disabled, returned
content URLs must match origin AND the exact job-content route. Responses,
dimensions, decoded bytes and times are bounded. No automatic POST retry.

Tauri commands: `easel_models` returns `{configured: bool, models: string[]}`;
`generation_submit {expectedRevision,model,prompt,mode}` returns Generation;
`generation_poll {id}` returns Generation; `generation_reject {id}` returns void;
`generation_accept {id,expectedRevision}` returns Snapshot.
`mode` is `generate` or `edit` (active cel PNG as reference).
`Generation = {id,status,model,prompt,document_id,base_revision,layer_id,frame_id,
width,height,pixels: number[] | null,error: string | null}`. Preview pixels are
deterministically nearest-neighbor resized to the document dimensions, with
this conversion disclosed in UI. Originals, receipt and provenance persist
under app-data/candidates before polling/acceptance. A post-202 storage failure retains the accepted
candidate with a warning and disables further submission; status retry persists
the receipt before any GET. Poll is explicit and
read-only; completed candidates remain immutable. Reject does not change document.
Accept requires the same document, revision, layer and frame and applies exactly
one `replace_cel` transaction. No job cancellation claim. 401/429/410 and timeouts
are actionable errors; unknown progress is indeterminate.

The client crate exposes its own typed Client/receipt API; native adapter maps
it to this IPC. Tests use a loopback fake HTTP server by default. Live calls are
authorized by this user, but never necessary for deterministic tests. MOSS is
not advertised without live registration plus generation proof.

## Agent boundary

Version-1 bounded tagged tool requests support inspect_document, get_palette,
add_layer, add_frame, paint_region, request_generation, preview_candidate and
apply_candidate. Read-only tools expose snapshots; mutations require explicit
approval and expected revision through the same core executor. Provider options
are Disabled, GPT, Ollama and llama.cpp; this sprint supplies a mock adapter,
not remote inference or arbitrary endpoint/path execution.

## Verification and deferred work

Core/HTTP/agent unit tests, frontend interaction tests, build, native Rust tests,
and a UI smoke run against real Rust are required. Preserve every existing
packaging job; add tests without replacing packaging. Full selection tools,
indexed-color editing, agent inference and MOSS adapters follow in sprint 002.
