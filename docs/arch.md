# Arch Linux packaging

MeePix now includes a **native Arch Linux** `meepix-git` development package, built from source with makepkg. It uses Arch's GTK3/WebKitGTK 4.1 libraries, rather than wrapping an Ubuntu AppImage.

## Install the prebuilt CI artifact

In a **successful** [MeePix Actions run](https://github.com/venetanji/meepix/actions), download `meepix-arch-x64` from **Artifacts**, extract the ZIP, and install the resulting pacman package:

```sh
sudo pacman -U ./meepix-git-*.pkg.tar.zst
```

The package installs `/usr/bin/meepix`, a desktop launcher, and icons. Run `meepix` or find MeePix in the application launcher. Remove it with `sudo pacman -R meepix-git`.

Do **not** install package artifacts from untrusted pull requests. These are development builds; they are not signed or reproducible yet. Pacman will handle dependencies from the configured Arch repositories.

## Build locally using makepkg

```sh
git clone https://github.com/venetanji/meepix.git
cd meepix/packaging/arch
sudo pacman -Syu --needed base-devel git
makepkg -si
```

`makepkg -si` installs missing dependencies through pacman (with the usual prompts), clones the public upstream repository, builds the React frontend and Rust/Tauri application, then installs the native package. Do **not** run `makepkg` as root. Builds download npm and Cargo dependencies; a local Rust toolchain and network access are required.

Alternatively, use the manually downloaded `.pkg.tar.zst` from GitHub Actions as above.

## Future AUR listing

`packaging/arch/PKGBUILD` and `packaging/arch/.SRCINFO` are AUR-oriented packaging files. **MeePix has not been published to AUR**. An AUR submission is a separate Git repository at `ssh://aur@aur.archlinux.org/meepix-git.git`, and requires an AUR account/authorized SSH key.

Before publishing, the repo owner should explicitly select and commit a MeePix `LICENSE`; the source currently does not declare one, and the recipe intentionally does not mislabel its license. Update PKGBUILD's SPDX `license` array and generate fresh metadata with:

```sh
cd packaging/arch
makepkg --printsrcinfo > .SRCINFO
```

Then audit package contents/runtime library dependencies with `namcap` and `ldd`, confirm naming doesn't collide with an existing AUR entry, and publish the PKGBUILD and .SRCINFO to the separate AUR repository. AUR helpers (`yay -S meepix-git`) work **only after** that submission.

## CI implementation

The GitHub Actions `arch-package` job builds in `archlinux:base-devel` on a fresh disposable container. It fetches the **exact checkout commit** through a temporary local Git branch for PR/push accuracy (never silently builds main in PR CI), invokes the same `PKGBUILD` as Arch users, verifies that the resulting pacman archive contains the binary and desktop entry, and uploads it for 14 days. The container performs no installations on the GitHub host or your machine.

Production packaging should add npm and Cargo lockfiles; the current dependencies are not reproducibly pinned. GPU inference/Easel are separately installed services and are **not** bundled.
