# MeePix

**An open-source, AI-native pixel art and sprite animation studio.**

MeePix aims to combine an editable sprite canvas, animation timeline, local image generation through [Easel](https://github.com/venetanji/easel) and ComfyUI, and a configurable AI agent (GPT or a local provider).

## Current status

The first functional slice has a Rust-owned 16/32/64/128 px RGBA document,
independent layers and animation frames, pencil/eraser/fill, zoom/pan,
previous-frame context, playback, undo/redo, lossless `.spriteproj` projects and
PNG sprite-sheet/JSON export. Easel generation and reference edits produce
reviewable candidates; acceptance is one reversible edit. The editor works
without an AI service. The typed agent/provider boundary is a tested skeleton;
provider inference and richer sprite tools remain follow-up work.

See [sprint evidence and limitations](docs/sprint-001-results.md) and
[Easel/model workflow follow-ups](docs/easel-workflow-followups.md).

## Development

Install a Rust toolchain, [Tauri 2 system prerequisites](https://v2.tauri.app/start/prerequisites/), Node.js (20 or later) and npm:

```bash
npm install
npm run tauri dev
```

To run the frontend alone: `npm run dev`. Artwork operations require the native
backend; there is no separate browser document implementation. Tests inject a
backend, and the browser smoke harness connects to the real Rust application.

Configure generation in the launching process with `EASEL_URL` and optionally
`EASEL_API_KEY` (never a `VITE_` variable). If only the key is set, the endpoint
is `https://easel.ait4x.org`. HTTP is accepted only for literal loopback addresses.
Tokens never enter frontend state or project files. Only Qwen-Image-2.1 is
currently offered by the editor; discovery alone is not proof of model readiness.

Projects and candidate originals/receipts are stored in the OS app-data directory
for `org.agenticmedia.meepix`, in `projects/` and `candidates/`. Project names use
letters, digits, hyphens and underscores. Saving another document over an existing
name is refused; load that project to edit it, or choose a new name.

Run `npm test`, `npm run build`, and `cargo test --manifest-path` for each of
`crates/sprite-core/Cargo.toml`, `crates/easel-client/Cargo.toml`,
`crates/meepix-app/Cargo.toml` and `src-tauri/Cargo.toml`.
`npm run test:browser` requires Cargo and Chromium (`CHROMIUM_PATH` may override
the executable). This optional smoke test binds a test-only Rust bridge to
loopback; it is not part of the desktop binary.

## Architecture

- **Tauri 2 / Rust:** authoritative sprite document engine, persistence and undoable mutations
- **React / TypeScript:** interactive editor and timeline interface
- **Easel / ComfyUI:** separately operated, locally configured generation backend
- **Agent:** GPT/cloud or local LLM, using the same validated editor tool protocol
- **Approval-first:** model outputs are candidate assets, not silent changes to artwork

See [architecture](docs/architecture.md) and [roadmap](docs/roadmap.md).

MeePix is intended to join the [Agentic Media](https://github.com/agentic-media) creative software ecosystem.

## Continuous integration

GitHub Actions builds native Arch Linux (`.pkg.tar.zst`), Linux (.AppImage), Windows (NSIS .exe), and macOS (Intel + Apple Silicon .dmg) packages on pushes to `main`, pull requests, and manual runs. Build artifacts are attached to each Actions run; nothing is automatically published. See [CI and build notes](docs/ci.md).

Run `npm run tauri -- build` locally to package the app (icons are generated automatically). These are early unsigned/notarization-free development builds; platform security prompts may apply.

### Arch Linux / Omarchy

MeePix can be installed as a native pacman package. Download a successful CI build's `meepix-arch-x64` artifact, extract it, and run `sudo pacman -U ./meepix-git-*.pkg.tar.zst`. To build from source, use the [PKGBUILD](packaging/arch/PKGBUILD); see the [Arch packaging guide](docs/arch.md). This is not yet published on AUR.
