# Releasing Video All-in-One

A release is made by pushing a tag `vX.Y.Z`. The workflow `.github/workflows/release.yml` then checks the tag, tests and builds on every OS, packages, signs the update artifacts, writes `latest.json` and publishes one GitHub Release. Pull requests are checked by `.github/workflows/ci.yml`.

Status of this document: the macOS packaging was run on a real Mac (Apple Silicon). Everything else described here was written but has not been run. The section "What is unproven" lists each part.

## What a release contains

| File | Platform key | Used for |
|------|--------------|----------|
| `video-all-in-one-X.Y.Z-macos-aarch64.zip` | `macos-aarch64` | Update artifact: the zipped `Video All-in-One.app` |
| `video-all-in-one-X.Y.Z-macos-x86_64.zip` | `macos-x86_64` | Update artifact |
| `video-all-in-one-X.Y.Z-macos-aarch64.dmg`, `...-macos-x86_64.dmg` | | First install by people (not an update artifact) |
| `video-all-in-one-X.Y.Z-windows-x86_64.exe` | `windows-x86_64` | NSIS installer; also the update artifact (run with `/S`) |
| `video-all-in-one-X.Y.Z-linux-x86_64.deb` | `linux-x86_64` | Package; also the update artifact (installed with `pkexec` and `dpkg -i`) |
| `<artifact>.sig` | | Ed25519 signature of the SHA-256 of the artifact |
| `latest.json` | | Update manifest, schema 1, with both languages of the notes |

The file names follow `video-all-in-one-<version>-<platform_key>.<ext>`; `cargo xtask latest-json` finds the artifacts by this pattern.

## One-time setup

Do these once, on your own machine. The agent that wrote the pipeline did not do them.

1. Generate the signing key pair, in a folder outside the repository:

   ```sh
   cargo xtask keygen --out ~/video-all-in-one-keys
   ```

   This writes `update-signing.key` (private, mode 0600, never overwritten) and `update-signing.pub` (public, base64), and prints the public key as the Rust constant `UPDATE_PUBLIC_KEY`.

2. Put the printed constant into `crates/platform/src/update/public_key.rs`. Build and commit it. Every app built from this commit trusts only this key.
   The production build reads the manifest address from the build variable `VAIO_UPDATE_MANIFEST_URL`, which `release.yml` sets to `https://github.com/<owner>/<repo>/releases/latest/download/latest.json` for the production build step. While `UPDATE_PUBLIC_KEY` is still all zeros, or the variable is missing, the app shows "Updates are not set up in this build." and downloads nothing.
3. Back up `update-signing.key` now (see "Key backup and rotation").
4. In the GitHub repository, Settings, Secrets and variables, Actions:
   - Secret `UPDATE_SIGNING_KEY`: the content of `update-signing.key`. Set it without printing it: `gh secret set UPDATE_SIGNING_KEY < ~/video-all-in-one-keys/update-signing.key`.
   - Variable (not a secret) `UPDATE_PUBLIC_KEY`: the content of `update-signing.pub`. It must be the same key as the constant in `public_key.rs`. `latest-json` refuses any artifact whose signature does not verify with it, so a wrong value stops the release; a value that differs from the compiled-in key would make a release that installed apps reject.
5. Protect release tags: Settings, Rules, Rulesets, New tag ruleset. Target the pattern `v*`. Turn on "Restrict creations", "Restrict updates" and "Restrict deletions", and allow only the repository owner to bypass. Without this, anyone with write access can publish a release.
6. Settings, Actions, General: set the default workflow permissions to read-only. The release workflow asks for `contents: write` only in its last job.

## How to release

1. Decide the version number from the size of the work (the project's release tag rule: propose it, wait for the user's answer).
2. Set the version in every file and write the notes:

   ```sh
   cargo xtask set-version X.Y.Z
   ```

   Add a section `## X.Y.Z` to `CHANGELOG.md` with `### English` first and `### Tiếng Việt` second. Both are required and placeholder text such as TODO is refused. These notes are what the update dialog and the GitHub Release show.
3. Check it locally:

   ```sh
   cargo xtask check-release --tag vX.Y.Z --latest <last published version, or none>
   cargo fmt --check
   cargo clippy --workspace --all-targets -- -D warnings
   cargo test --workspace
   ```
4. Commit as `chore(release): X.Y.Z`, open the pull request, wait for CI to be green on all four runners, merge.
5. With the user's permission, tag the merge commit and push the tag: `git tag vX.Y.Z && git push origin vX.Y.Z`.
6. Watch the workflow `Release`. The jobs are `check the tag`, then `build` on four runners, then `sign and publish`. The release is created as a draft, filled with all files, and published last.
7. Verify the published release: version, sizes and SHA-256 in `latest.json`, both note languages, a `.sig` for each artifact.
8. Before calling the release verified, do a fresh install, an upgrade from the previous version and an uninstall on a real machine or a clean VM of each OS, and write down the ones you could not test.

Build one package by hand on its own OS (after the production build):

```sh
cargo build --release -p vaio-desktop --features production
cargo xtask package --target macos   # or ubuntu; windows needs --libs <folder with libmpv-2.dll>
```

The output is in `target/package`. Never use `--workspace --features production`: Cargo would switch every crate to the production channel.

## App icons

The icons come from two SVG files: `assets/icons/app-icon.svg` (production) and `assets/icons/app-icon-dev.svg` (development, the same icon with an amber DEV band). To replace them, edit the SVGs (1024 x 1024, no text elements), run `cargo xtask icons` and commit the changed files in `assets/icons/generated/`. CI runs `cargo xtask icons --check` on Ubuntu and fails when the committed files differ from what the SVGs produce. The packagers use `generated/production/`: `icon.icns` becomes `AppIcon.icns` in the macOS bundle, `icon.ico` is the icon of the Windows installer and uninstaller, and `icon.png` is installed in the `.deb` as the `hicolor` 512x512 icon. The window icon of a running app is not set yet (GPUI only supports it on X11 and needs the `image` crate for it).

## First-run warnings

The project does not use paid certificates, so the first run shows an operating system warning. This is expected. Tell users these steps (the README links here).

- macOS: the app is signed only ad hoc and not notarized. After downloading, macOS says it cannot verify the app. On macOS 15 and later: open System Settings, Privacy and Security, scroll to the message about "Video All-in-One" and press Open Anyway. On macOS 13 and 14: Control-click the app, choose Open, then Open again. This is needed once; later updates installed by the app itself keep working.
- Windows: SmartScreen shows "Windows protected your PC". Press More info, then Run anyway. The installer does not need administrator rights; it installs for the current user into `%LOCALAPPDATA%\Programs\Video All-in-One`.
- Ubuntu: install with `sudo apt install ./video-all-in-one-X.Y.Z-linux-x86_64.deb`. No warning is shown.

## Where the in-app install falls back

If the app cannot install an update itself, it downloads and verifies the file and then shows it instead (the install fallback):

- macOS: when the app is not running from a `.app` bundle, or the folder that holds it is not writable by the user (for example a root-owned install). The `.dmg` first install and a copy in `~/Applications` or a user-writable `/Applications` use the in-app path.
- Ubuntu: when `pkexec` is not available.
- Windows: there is no fallback; the installer runs silently with `/S`.

## Key backup and rotation

- The private key exists in two places only: the GitHub secret and your backup (for example a password manager). GitHub never shows a secret again.
- If the key is lost, installed apps can never update again. People would have to download a new build by hand, and that build would carry a new public key.
- There is no rotation mechanism. Changing the key means shipping a build with the new public key that is signed with the old key first; do that deliberately and never in the same release as other risky changes. A leaked key means anyone can publish an update that installed apps accept, so treat a leak as an incident: rotate as above and tell users.
- Never print the key in a log, never commit it, never pass it on a command line. `cargo xtask sign` reads it from the environment `UPDATE_SIGNING_KEY`, and only the signing step of the release workflow has that variable.

## Pinned inputs and how to update them

| Input | Where | How to update |
|-------|-------|---------------|
| GitHub Actions (`actions/checkout`, `cache`, `upload-artifact`, `download-artifact`) | full commit SHA with the version in a comment, in both workflow files | Open the action's release page, copy the tag's commit SHA, replace it in both files |
| Rust compiler | `RUSTUP_TOOLCHAIN` at the top of both workflow files (overrides `rust-toolchain.toml`, which says `stable`) | Change both to the same new version in one pull request |
| Runner images | `macos-15`, `macos-15-intel`, `windows-2025`, `ubuntu-22.04` | Change the matrix when a runner is retired |
| libmpv for Windows | `MPV_WIN_URL` and `MPV_WIN_SHA256` in both workflow files; LGPL build of `zhongfly/mpv-winbuild` | Pick a newer release, copy the SHA-256 shown on its page, change both values in both files |
| libmpv and FFmpeg for macOS | Homebrew, not pinned: `brew install mpv ffmpeg` installs whatever is current | The app bundle contains the libraries of that day. To pin, build mpv and FFmpeg from source or use a fixed bottle (not done) |
| libmpv for Ubuntu | `libmpv-dev` and `ffmpeg` from Ubuntu 22.04 (libmpv 0.34) | Changes only with Ubuntu updates |
| FFmpeg, FFprobe, yt-dlp | Not bundled yet. They come from the `PATH` (sprint 4 bundles and pins them with checksums and licence notices) | |

## Licences

- The Windows build links the LGPL build of libmpv.
- The macOS bundle contains the Homebrew build of libmpv and FFmpeg, which is built with GPL components (for example x264 and x265). Shipping those inside an MIT-licensed app needs a licence decision and a notice file before a public release. This pipeline does not add notice files yet.
- The Ubuntu package depends on the system libmpv and does not ship it.

## Known limits

- macOS minimum version: the bundled Homebrew libraries are built for the macOS version of the build machine (macOS 15 on the CI runners). `cargo xtask package` reads the real minimum from every file and writes the highest one into `LSMinimumSystemVersion`, so older systems get a clear refusal instead of a crash. The README therefore promises only a recent macOS. Supporting macOS 13 needs a libmpv built for macOS 13 (a later task).
- Ubuntu: the package depends on `libmpv1`, which exists on 22.04 only. Ubuntu 24.04 ships `libmpv2` and cannot install this package.
- The Ubuntu package lists only `libmpv1` and `libc6` as dependencies. The other libraries GPUI loads at run time (Wayland, X11, Vulkan, fontconfig, ALSA) are not declared; the CI step runs `ldd` to catch missing ones on the runner only.
- The `.deb` maintainer address is `noreply@example.invalid` until a real contact exists.

## What is unproven

Run on a Mac (Apple Silicon): `cargo xtask package --target macos` builds the app bundle, the zip and the dmg; the bundle starts and stays alive for six seconds with `DYLD_LIBRARY_PATH` empty; `otool -L` of the executable and of every bundled library shows no path under `/opt/homebrew` or `/usr/local`; the ad-hoc signature verifies.

Not run anywhere:

- Both workflow files on GitHub. They were checked as YAML only. Every step is unproven until a pull request and a release run.
- Intel macOS packaging (same code, other architecture).
- The Windows package: `makensis` was not available, so the generated NSIS script was unit tested as text only. The `lib.exe` step that makes `mpv.lib`, the assumption that the libmpv package contains `mpv.def` and `libmpv-2.dll`, linking the app against it, and the silent install and uninstall steps.
- The Ubuntu package: `dpkg-deb` was not available, so the control file and desktop entry were unit tested as text only. Building the `.deb`, installing it, the `ldd` check and removal.
- `cargo xtask icons --check` on Ubuntu compares bytes with files made on a Mac. If the renderer rounds differently on another CPU, the check fails there and the icons must be regenerated or the check relaxed.
- Fresh install, upgrade and uninstall on a real machine or clean VM for any OS, including the updater installing a package made by this pipeline.
- Notarization: not done, by design.
- `UPDATE_SIGNING_KEY`, `UPDATE_PUBLIC_KEY` and the tag ruleset are not set up; the key has not been generated.
