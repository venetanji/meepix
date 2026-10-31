# MeePix implementation roadmap

## Sprint 1 — desktop foundation

- [x] Initial Tauri + React + TypeScript shell (prototype only)
- [x] Basic frontend-only 32x32 pencil/eraser demo (not persisted)
- [ ] Rust sprite document model, layers, frames, cels and revision IDs
- [ ] Typed document editing commands with undo/redo
- [ ] Move canvas operations into Rust and render composited output
- [ ] Save/load project file with lossless round-trip tests

## Sprint 2 — animation and assets

- [ ] Timeline, frame durations, onion skin and playback
- [ ] Import/export PNG and sprite sheets with JSON metadata
- [ ] Palette quantization and per-cel alignment

## Sprint 3 — generation

- [ ] Configurable loopback Easel endpoint and health/model discovery
- [ ] Submit async generation/edits, poll stored job receipts, retrieve results
- [ ] Candidate selection and user-approved undoable import
- [ ] Add/verify MOSS-Image Easel adapter; use existing Qwen builder

## Sprint 4 — agent

- [ ] Provider-agnostic GPT/local tool-calling adapter
- [ ] Document inspection tools, scoped asset requests and approval gates
- [ ] End-to-end sprite revision acceptance test

## Notes

The scaffold's Easel button is explicitly a placeholder, not a working integration. Avoid implying animation, project persistence, or model generation exists until tests demonstrate it.
