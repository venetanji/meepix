# MeePix architecture (draft 0.1)

MeePix is a local-first pixel art and sprite animation editor. The first native vertical slice implements the Rust document engine, snapshot-driven animation UI, local project storage and a bounded Easel proposal manager. See [sprint contracts](sprint-001-contracts.md) for the concrete IPC and [evidence](sprint-001-results.md) for tested scope and remaining limitations.

## Boundaries

1. **Rust core** owns layered RGBA cels, frame timing, palette data, project saves and undoable transactions. Avoid coupling it to Tauri or provider SDKs.
2. **React UI** renders editor state and handles pointer interactions, selection, zoom and panels. It should never become a second authoritative document store.
3. **Easel REST** connects to a separately operated local Easel gateway and ComfyUI service. Use Easel's existing `/v1/images/generations`, `/v1/images/edits` and async job receipt API rather than building a second ComfyUI client.
4. **Agent provider** is configurable: GPT via API, Ollama, llama.cpp, or another supported endpoint. Cloud providers receive only user-approved document context, references and screenshots. A local model is needed for fully offline operation.
5. **Agent commands** are typed and validated Rust transactions. AI generation produces an immutable candidate; applying it to a frame requires user acceptance.

## Important Easel constraints

- `Prefer: respond-async` gives a durable accepted job receipt with an ID; poll `GET /v1/images/jobs/{id}` and fetch completed content.
- Receipts and cached outputs expire after 24 hours. Store accepted assets within the MeePix project.
- No image job cancellation API or idempotency for ambiguous POST submissions is currently available; do not silently retry.
- Easel already supports Qwen-Image-2.1 generation and edits. MOSS integration must be verified and, if absent, added in Easel.

## Initial acceptance scenario

A user sketches a 64x64 sprite, adds animation frames, selects an area, requests a replacement with the chosen agent, receives 3 candidates from local Easel, approves one, and exports an editable animation and sprite sheet.
