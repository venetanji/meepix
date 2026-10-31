# Sprint 001 evidence

Base: `9952d9b` (issue #1). Work was split across repository-configured Terra
implementer, independent tester and reviewer roles, capped at two workers plus
coordinator. The coordinator integrated the IPC, native service, agent tools and CI.

## Implemented

- Independent Rust sprite core: bounded RGBA cels, stable IDs, alpha compositing,
  layers, cloned independent frames, timing, palette, monotonic revisions and
  reversible transactions. Rust owns all artwork; JS holds returned snapshots.
- Pointer strokes/eraser/fill, grid/checkerboard, zoom/pan, layer visibility,
  timeline/playback and previous-frame context, dirty/error feedback.
- Versioned ZIP+PNG project saves, validated load, PNG sheet/JSON export. Writes
  stage in the destination directory. New names are unique and unrelated existing
  projects cannot be overwritten. Closing dirty work requires confirmation.
- Native Easel adapter with environment-only credentials, configured origin,
  model discovery, async submit-once, polling, authenticated content, durable
  receipt/original/preview/provenance, explicit reject or one undoable acceptance.
- Version-1 typed tool executor with bounded core mutations, separate trusted
  approval argument, revision validation and disabled/GPT/Ollama/llama.cpp provider
  configuration plus mock provider. No live agent inference is implemented.

[Browser smoke capture](images/editor-browser.png) · [Native edited window](images/editor-native.png)

## Actual local checks

| Command/check | Observed result |
| --- | --- |
| `npm test` | 14 tests pass |
| `npm run build` | TypeScript and production Vite build pass |
| `cargo test --manifest-path crates/sprite-core/Cargo.toml` | 9 integration tests pass |
| `cargo test --manifest-path crates/easel-client/Cargo.toml` | 11 fake-HTTP tests pass |
| `cargo test --manifest-path crates/meepix-app/Cargo.toml` | 10 application integration tests pass |
| `cargo test --manifest-path src-tauri/Cargo.toml` | Native crate compiles; its unit/doc suites contain zero tests |
| `npm run test:browser` | 1 Chromium smoke against real Rust application bridge passes |
| `cargo fmt ... --check`, `git diff --check` | Pass |
| Native Tauri window on Xvfb | Pointer-drag painting, 2 layers, 4 frames, Save; saved ZIP manifest inspected |
| Authorized live Qwen smoke | One 202 POST, completed poll, authenticated valid PNG retrieval |

The browser test checks actual Rust pixels at both canvas edges, undo/redo,
layer/frame counts, playback and lossless save/reload/export. It substitutes only
IPC transport, not document logic. Fake HTTP tests cover states, 401/429/410,
timeouts, malformed PNG, bounded retrieval, redirects/no auth leak, multipart edit
and no automatic POST retry. Independent tests cover stale/changed targets,
rejection, undoable acceptance, corrupted projects and injected storage failures.
Review found and fixed UI lifecycle races, lost poll recovery, target selection,
unsaved-close/name risks and post-202 storage recovery with no resubmission.

This cloud image initially lacked Rust and GTK development packages. Native
checks used a local toolchain and extracted Debian sysroot under ignored `work/`.
WebKit helper paths were relocated in that unshipped sysroot and its process
sandbox disabled for the container smoke only; application production defaults
were not changed. No API key was printed, stored in project files or committed.

## CI and remaining gaps

Existing Arch pacman, AppImage, Windows NSIS and Intel/ARM macOS jobs remain.
Interaction/core/client/application tests were added without removing packaging;
core persistence tests also run in the desktop platform matrix. Remote results
belong to the draft PR checks; local tests do not prove Arch or other platforms.

1. Inspect all draft-PR platform package checks before considering merge; no
   release, deployment or merge is authorized by this sprint.
2. Add stored-job listing/resumption across app restarts and richer multi-candidate
   selection. Originals and receipts are durable now, but discovery/recovery UI
   is session-scoped. Poll retry is GET-only; there is no cancellation endpoint.
3. Add scoped selection/masks, layer reorder/delete, frame deletion/duplication,
   configurable palette management, indexed color and asset alignment/quantization.
4. Flesh out provider configuration UI and GPT/local inference behind validated,
   explicit-approval tool proposals. The editor requires none of these providers.
5. Validate live Qwen reference edits and MOSS workflows separately. No MOSS
   availability claim is made; see [model/workflow handoff](easel-workflow-followups.md).
6. Improve dirty-region rendering/interactive stroke preview and animation thumbnails.
   Small snapshots are currently returned per completed transaction.
7. PNG and JSON sheet files cannot be jointly crash-atomic; each is staged and
   ordinary failure rollback is tested. Cross-platform failure semantics need CI
   and Windows desktop verification. No independent native packaging was run locally.
