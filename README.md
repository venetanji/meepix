# MeePix

**An open-source, AI-native pixel art and sprite animation studio.**

MeePix aims to combine an editable sprite canvas, animation timeline, local image generation through [Easel](https://github.com/venetanji/easel) and ComfyUI, and a configurable AI agent (GPT or a local provider).

## Current status

**Early prototype:** this repository contains a Tauri 2 + React/TypeScript desktop shell and a 32 × 32 frontend-only pencil/eraser canvas. Layers, animation, saving, undo/redo, generation, and agent tools are **planned, not yet implemented**. The Easel check button is currently a stub.

## Development

Install a Rust toolchain, [Tauri 2 system prerequisites](https://v2.tauri.app/start/prerequisites/), Node.js (20 or later) and npm:

```bash
npm install
npm run tauri dev
```

To run the frontend alone: `npm run dev`.

The initial scaffold is committed but **has not yet been compiled or run in CI**. The first engineering gate is to build it and resolve platform-specific Tauri dependencies.

## Architecture

- **Tauri 2 / Rust:** future authoritative sprite document engine and undoable mutations
- **React / TypeScript:** interactive editor and timeline interface
- **Easel / ComfyUI:** separately operated, locally configured generation backend
- **Agent:** GPT/cloud or local LLM, using the same validated editor tool protocol
- **Approval-first:** model outputs are candidate assets, not silent changes to artwork

See [architecture](docs/architecture.md) and [roadmap](docs/roadmap.md).

MeePix is intended to join the [Agentic Media](https://github.com/agentic-media) creative software ecosystem.

## Continuous integration

GitHub Actions builds Linux (.deb), Windows (NSIS .exe), and macOS (Intel + Apple Silicon .dmg) installers on pushes to `main`, pull requests, and manual runs. Build artifacts are attached to each Actions run; nothing is automatically published. See [CI and build notes](docs/ci.md).

Run `npm run tauri -- build` locally to package the app (icons are generated automatically). These are early unsigned/notarization-free development builds; platform security prompts may apply.
