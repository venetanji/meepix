# MeePix continuous integration

The workflow is at [`.github/workflows/ci.yml`](../.github/workflows/ci.yml).

It borrows the read-only permission model, pinned GitHub actions and independent jobs from the [Ultra Tauri CI](https://github.com/venetanji/alive/blob/main/.github/workflows/ultra-ci.yml), but builds MeePix itself rather than Ultra's audio dependencies.

## Triggers and build matrix

- Pull requests, pushes to `main`, and manual `workflow_dispatch`.
- Ubuntu 24.04 x64 — frontend compile + Rust tests, and a separate Arch Linux container job producing native `.pkg.tar.zst`. Ubuntu 22.04 x64 — portable `.AppImage` build (older glibc baseline).
- Windows Server 2022 x64 — NSIS installer `*-setup.exe`.
- macOS 15 Apple Silicon and macOS 15 Intel — `.dmg` bundles for both CPU architectures.

Each build, including the native Arch package, is a GitHub Actions **workflow artifact** retained for 14 days. Download it from the successful run's **Artifacts** section. No GitHub Release is created and no upload keys, Apple certificates or external model services are needed. CI intentionally has `contents: read` permissions and checkout does not persist credentials.

macOS builds use ad-hoc signing (`APPLE_SIGNING_IDENTITY=-`), not Apple Developer notarization. Windows installers are unsigned. Treat all artifacts as development previews.

## Running on Arch / Omarchy

**Preferred:** Install the [native pacman package](arch.md) from the `meepix-arch-x64` artifact using `sudo pacman -U ./meepix-git-*.pkg.tar.zst`, or build it locally with `makepkg -si`.

**Alternative portable bundle:** Download the AppImage workflow artifact, unzip the artifact, then run:

```sh
chmod +x MeePix*.AppImage
./MeePix*.AppImage
```

The executable name depends on the Tauri bundle version; use the `.AppImage` file you downloaded. Some AppImage runtimes require FUSE 2 compatibility on the host. For testing without FUSE mounting you may try `APPIMAGE_EXTRACT_AND_RUN=1 ./MeePix*.AppImage`. This is a development build, not a distro-tested or universally portable release.

## Local build

Install Node.js 22, Rust stable, and the [Tauri 2 platform prerequisites](https://v2.tauri.app/start/prerequisites/). On Linux use the matching WebKit2GTK 4.1 packages.

```sh
npm install
npm run icons                 # optional: npm's pretauri hook does this automatically
npm run build
cargo test --manifest-path src-tauri/Cargo.toml
npm run tauri -- build        # platform installer(s)
```

The source application icon is `src-tauri/icons/source.svg`. The `icons` script generates Tauri's platform icon files; do not commit those generated files. The SVG is intentionally temporary.

## Follow-ups / reproducibility

The repository scaffold currently has **no `package-lock.json` or `Cargo.lock`**. CI therefore uses `npm install` and a normal `cargo test/build`, rather than claiming reproducible locked builds. Generate and commit both lockfiles before introducing release or supply-chain attestations, then switch CI to `npm ci` and Cargo `--locked`. Prefer checksum-pinned dependencies and review any changes to workflow permissions.

This CI checks **compilation and packaging**, not graphical interaction, animation behavior, or GPU inference. As the sprite engine matures, add deterministic Rust tests, browser canvas tests, and mock Easel/agent contract tests before real GPU integration gates.
