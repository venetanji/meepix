# MeePix — coding agent instructions

MeePix is a local-first, open-source, AI-assisted pixel-art and frame-animation application for game assets. Product design, architecture and phases are in [docs/architecture.md](docs/architecture.md), [docs/roadmap.md](docs/roadmap.md), and [docs/codex-sprint-001.md](docs/codex-sprint-001.md). The existing 32x32 React drawing prototype and Tauri 2 shell are only a starting point.

## Work boundary

- **Rust is authoritative:** sprite pixels, RGBA cels, layers, frame durations, palette, revision IDs, transactions, undo/redo and save/load. Keep sprite-core independent of Tauri, Easel and agent providers.
- **React/TypeScript handles interaction only:** editor canvas, coordinate conversion, timeline, selection, panels and previews. Do not create a second authoritative document store. Design for 16/32/64/128 px assets and nearest-neighbor rendering.
- **Easel handles generation:** reuse the existing Easel REST API from [venetanji/easel](https://github.com/venetanji/easel) instead of rebuilding ComfyUI graph transport. Use `GET /v1/models`, `POST /v1/images/generations`, `POST /v1/images/edits`, `Prefer: respond-async`, `GET /v1/images/jobs/{id}`, and authenticated job content retrieval.
- **AI is optional:** agent reasoning may be GPT/cloud or local via Ollama/llama.cpp; image generation uses user-selected Easel/ComfyUI service. The editor must still work with none of these services configured.
- **Approval-first:** generated outputs are immutable candidates, never automatically applied. Apply an accepted result as one undoable transaction. Reject leaves the document unchanged.
- **No new generative model claims:** Qwen-Image-2.1 is implemented in Easel. Do not imply MOSS is available until model registration and a real test verify it. Keep model weights/licenses separate from MeePix's own code.

## Roles and collaboration

The coordinating agent is **GPT-6.1 Sol**, with subagents configured in `.codex/agents/` on **GPT-6.1 Terra**. Delegate independent, file-disjoint changes to `terra_implementer`; after each change use `terra_tester` and `terra_reviewer` as independent gates. Cap concurrency to 3 and do not run multiple write-heavy agents in the same file.

The coordinator owns interface decisions, merge/conflict resolution, end-to-end tests and PR summaries. Use small commits on a feature branch, not direct pushes to `main`. Do not merge, release, deploy, transfer repositories or publish an AUR package without explicit approval.

## Safety

- Never expose or commit `EASEL_API_KEY`, OpenAI API keys, other environment values, bearer tokens or generated credentials. An injected key is available to the running process, not to frontend source, build artifacts, test snapshots or logs.
- Check connection and models without triggering generation. Use a fake Easel transport by default; ask before live GPU/paid tests. Never retry an image-generation POST after an ambiguous network failure: Easel currently has no idempotency mechanism.
- Store accepted generated assets into project storage promptly; Easel job receipts/output expire after 24 hours. Do not claim image cancellation: Easel's current async image job API has no cancellation endpoint.
- Respect user-created artwork, uncommitted changes, contributor licenses, and existing pipelines. Validate dimensions, input bytes, paths and resource limits.
- No automatic destructive actions or changes outside this repository.

## Completion evidence

Report: code changed, actual commands/tests and their outcomes, CI status, working UI preview where possible, remaining gaps, secrets/use warnings, and PR link. Never substitute documentation for implementation or claim visual/desktop tests without running them.
