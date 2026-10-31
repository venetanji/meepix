# MeePix development — Codex sprint 001

> **For the Sol orchestrator with Terra workers/testers.** Read [AGENTS.md](../AGENTS.md), [architecture](architecture.md), [roadmap](roadmap.md), and existing source before coding. This sprint establishes a genuinely editable, persistent animation document and the first Easel proposal workflow.

## Desired user experience

A user launches a functional Tauri pixel editor on Arch, creates a 64x64 sprite, draws on two RGBA layers, makes four animation frames, plays the animation, saves and reloads it losslessly, requests an AI asset edit via a configured Easel endpoint, views returned candidates and rejects or accepts one; accepting creates a reversible edit on a chosen layer/frame. No configured Easel or agent provider must be required for manual drawing.

## Fixed architecture

- Desktop: Tauri 2, Rust, React 19, TypeScript, Vite; preserve Arch native package + AppImage + Windows/macOS CI.
- Core: Rust authoritative sprite document (independent crate if useful); stable frame/layer/cel identifiers, RGBA pixel data, frame duration, palette swatches, revision number, undo/redo transactions, save/load. A `.spriteproj` ZIP container is intended; start with a versioned manifest and PNG cel payloads, round-trip tests and atomic saves.
- UI: pointer-drag pencil/eraser/fill, zoom and pan, visible grid/checkerboard, palette, layers and timeline with frame playback. Efficient dirty-region or composited image update path; no second mutable document in JS.
- Generation: Easel client in Rust, configured endpoint and credentials, a fake test server, health/model discovery, async submit, poll and authenticated fetch, persist receipt, content and provenance, candidate preview, explicit accept/reject. Protect auth secrets from rendering and logs.
- Agent: provider abstraction and typed tool-command protocol, with GPT and local LLM options; initial mock/adapter skeleton is sufficient this sprint. Never let the LLM mutate document state without Rust validation and appropriate user approval.
- Source models: existing Easel Qwen graph for edits and generations. MOSS may be a future Easel adapter; do not block the first editor slice on MOSS registration or GPU setup.

## Execution order and ownership

### Stage 0: inspect and plan

1. Record current `main` commit and CI status. Check any uncommitted state.
2. Set up a feature branch. Locate existing files; independently inspect Easel's `README.md`, `IMAGE_JOBS.md`, `easel/qwen_graph.py`; consult creative-skills as workflow reference where relevant.
3. Root Sol defines the typed contracts for Rust document commands, composited-frame output, and Easel job state before parallel implementation starts. Store decisions in a brief design note in this PR.
4. Delegate file-disjoint tasks to Terra agents. Don't spawn more agents merely to create artificial parallelism.

### Stage 1: native document engine — Terra implementer A

- Files: `crates/sprite-core/**` (or isolated Rust module), tests there, thin Tauri command handlers in `src-tauri/src/**`.
- API: document create/read, set pixel/brush stroke, layers, cels, frame add/select, duration, snapshot/compositing, revision checks and transaction undo/redo.
- Tests: pixel coordinates at boundaries, transparent RGBA, independent frames, linked-cel policy, action reversibility and stale revision failures.
- Follow-on contract to UI worker: explicit Rust command names/types; no speculative IPC changes.

### Stage 2: editor UI — Terra implementer B

- Files: `src/**`, style/tests; coordinate with Stage 1 over exact IPC types.
- Build canvas with scalable nearest-neighbor display, drawing, layers and frame timeline, playback, palette swatches, persistent unsaved state indicators and error feedback.
- Test interaction: drag strokes without skipped points, edge clicks, eraser transparency, layer/frame selection, undo/redo and zoom/pan mapping.
- First create a mock `DocumentBackend` interface so frontend tests can run without Tauri; integrate Rust commands when stable.

### Stage 3: save/export — Terra implementer A after Stage 1

- Implement versioned local save/reload and PNG/sprite-sheet export plus JSON frame metadata.
- Test exact round trips of multiple cels, layer order and durations, path validation, atomic write failure and corrupted project error handling.

### Stage 4: Easel client + proposal UI — Terra implementer C

- Files: isolated `src-tauri/src/easel/**` and mock HTTP tests. Coordinate UI candidate-preview surface with B.
- Validate configured origins, avoid token exposure, use timeouts, `Prefer: respond-async`, persist job receipt before polling, fetch authenticated content, and write accepted output locally.
- Handle `queued`, `in_progress`, `completed`, `failed`, `cancelled`; unknown progress remains indeterminate. Expired job is an explicit error. No cancellation or blind retry.
- Generate candidate preview using fake Easel first; applying selected candidate must be one reversible document mutation; rejecting has zero side effects. Real inference is a separate user-authorized smoke test.

### Stage 5: agent tools boundary — Sol + Terra implementer

- Define a versioned, bounded tool schema: `inspect_document`, `get_palette`, `add_layer`, `add_frame`, `paint_region`, `request_generation`, `preview_candidate`, `apply_candidate`.
- Add a tool executor that validates arguments and revision. Ensure no LLM-supplied arbitrary file paths or endpoints are executed.
- Add configurable provider interface. Do not require an OpenAI SDK or remote access to build or run the editor. Provider-specific inference and visual verification may follow in sprint 002.

### Independent verification — Terra tester + reviewer

- After each stage, have tester run real unit/integration tests and verify behavior against stage acceptance, and reviewer inspect architecture, security and failure paths.
- Ensure `npm run build`, `cargo test --manifest-path src-tauri/Cargo.toml` and existing GitHub Actions pass. Add stable visual/browser smoke tests where available; do not fake graphical proof.
- Run unconfigured Easel/agent negative tests, timeout/401/429/410 tests, stale revision, rejection, save failures and no-auth-leak tests.
- Review against `docs/architecture.md`, record any shortcomings as follow-up issues.

## End-of-sprint pass/fail

- [ ] Tauri app runs with manual pencil/eraser on actual Rust document data.
- [ ] At least 2 layers, 4 frames, durations, onion skin or equivalent frame context, and playback work.
- [ ] Undo/redo restores exact bytes and save/reload preserves pixels, layers and frames.
- [ ] PNG sprite sheet and frame metadata export without out-of-bounds/alpha errors.
- [ ] Fake Easel async generation completes; candidate can be accepted (undoable) or rejected (no mutation).
- [ ] Absent/malformed Easel config never prevents manual editing.
- [ ] A minimal typed agent tool executor is tested, without executing arbitrary requests.
- [ ] CI checks and Arch build are green, with actual results recorded.
- [ ] Root Sol produces one draft PR with summaries, screenshots if available, tests, known limitations and the next sprint proposal. Do **not** merge.

## Resource and confidentiality policy

Keep worker count to the project configuration. No secrets in prompts, output, source, UI state, logs, screenshots or artifacts. Local test APIs and mocks first. External Easel endpoint credentials, if provided in the Codex environment, are for authorized API calls, not for exfiltration, printing, publishing or committing. Confirm permission before any nontrivial real GPU request or paid service usage. Do not initiate indefinite work or repeatedly submit jobs.

## After this sprint

Subsequent PRs: improved sprite tools and indexed-color mode; richer animation editing, image alignment/palette quantization, agent provider selection with GPT + local LLM, Qwen reference edit and MOSS workflow validation, platform packaging and stable release preparation.
