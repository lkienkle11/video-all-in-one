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

The file names follow `video-all-in-one-<version>-<platform_key>.<ext>`; `cargo xtask latest-json` finds the artifacts by this pattern. The development build's files, `video-all-in-one-<version>-dev-<platform_key>.<ext>`, are never part of this release (see "The development build and the dev release").

## The development build and the dev release

Every release also builds the development app (no `production` feature) from the same commit, and publishes it as a second, separate GitHub pre-release. It is for testers.

| | Production | Development |
|---|---|---|
| App name | `Video All-in-One` | `Video All-in-One (Dev)` |
| Identifier and data folder | `app.videoallinone` | `app.videoallinone.dev` |
| Executable and package name | `video-all-in-one` | `video-all-in-one-dev` |
| Icon | `assets/icons/generated/production/` | `assets/icons/generated/development/` (amber DEV band) |
| Windows | `Programs\Video All-in-One`, uninstall key `app.videoallinone` | `Programs\Video All-in-One (Dev)`, uninstall key `app.videoallinone.dev`, own Start-menu entry |
| macOS | `Video All-in-One.app` | `Video All-in-One (Dev).app` |
| Ubuntu | package `video-all-in-one`, `/usr/lib/video-all-in-one` | package `video-all-in-one-dev`, `/usr/lib/video-all-in-one-dev`, own desktop entry |
| Release file | `video-all-in-one-X.Y.Z-<platform_key>.<ext>` | `video-all-in-one-X.Y.Z-dev-<platform_key>.<ext>` |
| Updates | checks and installs updates | never checks for updates |

So both can be installed side by side with separate settings and data.

- **Build by hand:** `cargo build --release -p vaio-desktop` (without `--features production`), then `cargo xtask package --channel dev --target <os> ...` (the default is `--channel production`). On Ubuntu link with the run path `$ORIGIN/../lib/video-all-in-one-dev`. A dev package is an installer only (`.dmg`, `.exe`, `.deb`): no update zip, and it is never signed with the update key.
- **Tag and release:** the dev release is the pre-release `dev-vX.Y.Z` (title `Video All-in-One X.Y.Z (development build)`), created by the `sign and publish` job after the production release. The tag is `dev-vX.Y.Z` and not `vX.Y.Z-dev` because the workflow runs on every pushed tag `v*`; a tag starting with `v` would start it again. It is created with `--prerelease --latest=false`, so it is never the "latest" release: `releases/latest/download/latest.json` keeps pointing at the production release. It has no `latest.json`.
- **`latest-json` and signing ignore every `-dev-` file.** The production release never contains one, and `--require` still fails when a production artifact is missing even if its dev file exists.
- **How testers install it:** open the Releases page, pick the pre-release `dev-vX.Y.Z`, download the file for the OS (`.dmg`, `.exe` or `.deb`) and install it by hand like any first install (see "First-run warnings"). It does not update itself: to get a newer dev build, install the next `dev-v...` pre-release.
- **Verify** (not yet run on a real release): a manual `workflow_dispatch` run builds both sets of artifacts and publishes nothing; after a real release check that `dev-vX.Y.Z` is a pre-release with four files and that `releases/latest/download/latest.json` still resolves to the production release.

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

## Upgrade checks

Run the upgrade checks in `docs/testing/clean-machine-checklist.md` as part of every release on each supported system. The first Windows build is also the first time the installer script is compiled, so the release manager watches that step. During an update, macOS keeps its backup in a hidden `.vaio-update-*` folder next to the app, Windows uses `.previous`, and Ubuntu has no backup folder because `dpkg` manages the files.

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
cargo xtask package --target macos --libmpv-dir <folder of our libmpv build>   # or ubuntu; windows needs --libs <folder with libmpv-2.dll>
```

The output is in `target/package`. macOS and Ubuntu packages need `--libmpv-dir` (see "Our own libmpv"). Never use `--workspace --features production`: Cargo would switch every crate to the production channel.

## Our own libmpv (macOS and Ubuntu)

Release packages bundle only LGPL builds of libmpv and FFmpeg. Homebrew's builds are GPL (x264, x265), and Ubuntu 22.04 has only `libmpv1` (mpv client API 1), which the app refuses at run time. So we build our own libmpv once per platform and publish it:

- What is built: FFmpeg, FreeType, FriBidi, HarfBuzz, libass, libplacebo (without any GPU API; its release archive has no git submodules, so the sources table also pins `fast_float` and the Vulkan headers and unpacks them into libplacebo's `3rdparty` folder) and mpv, all shared, all from the source archives pinned in one table in `xtask/src/libmpv/sources.rs` (https URL and SHA-256 per archive; the hash is checked before anything is unpacked). The flags are in `xtask/src/libmpv/recipe.rs`: FFmpeg with `--disable-gpl --disable-nonfree --disable-programs --disable-autodetect` and only native decoders (VideoToolbox on macOS); mpv with `-Dgpl=false -Dlibmpv=true -Dcplayer=false`, no Lua, no JavaScript, no GL or Vulkan, ALSA and PulseAudio on Linux, CoreAudio on macOS.
- Reruns: each installed component leaves a marker `DIR/.markers/<name>` with the hash of its recipe (version, source hash, flags, target). A build into a folder that has markers continues and skips every component whose marker matches; `build-libmpv.yml` caches that folder per runner (`actions/cache/restore` and `save`, also after a failure). The markers are not packed. Python modules `jinja2` and `markupsafe` for libplacebo's shader templates come from the pinned virtual environment of the workflow.
- Commands: `cargo xtask libmpv build --target ubuntu|macos --out DIR` and `cargo xtask libmpv pack --dir DIR --out FILE.tar.gz` (prints the SHA-256). The build needs `meson`, `ninja`, `pkg-config`, `make`, a C compiler, `curl`, `tar`, `nasm` on Intel CPUs, and `patchelf` on Linux; it says which one is missing. Every build tool starts with a clean environment.
- The folder `DIR`: `lib` (shared libraries with the run path `$ORIGIN` on Linux and `@loader_path` and the install name `@rpath/<file>` on macOS), `include`, `lib/pkgconfig` (relocatable), `LICENSES` (licence texts of every component and `SOURCES.txt` with the source URLs, needed for LGPL compliance) and `MANIFEST.txt` (component, version, source SHA-256, flags). The command ends by running the licence scan of the packager over every library, checking that `mpv_render_context_create` exists, and compiling a probe that must print client API major version 2.
- Workflow `build-libmpv.yml` (manual, or a push to `feat/**` that changes the recipe) builds on `ubuntu-22.04`, `macos-15` (arm64) and `macos-15-intel` (x86_64) and adds each archive `libmpv-<recipe>-<platform>.tar.gz` to the pre-release `libmpv-<recipe>` (never a `v*` tag). It never overwrites an asset: change `RECIPE_VERSION` in `sources.rs` and `LIBMPV_RECIPE` in the three workflow files together (a test checks they agree) to publish a changed build. The SHA-256 of each archive is written to the job summary.
- Pinned hash: `ci.yml` and `release.yml` have `LIBMPV_SHA256_LINUX_X86_64`, `LIBMPV_SHA256_MACOS_ARM64` and `LIBMPV_SHA256_MACOS_X86_64` at the top. They say `PENDING-first-build` until the first build is published, and the download step fails with a clear message while they do. After the first build, copy each printed SHA-256 into both files. The step downloads the archive, compares the SHA-256 and only then unpacks it to `$RUNNER_TEMP/libmpv`, and sets `PKG_CONFIG_PATH`, `LIBRARY_PATH` and `RUSTFLAGS` (`-L native=DIR/lib` and a run path to it) for the build and the tests.
- Rebuild or update: change the row of the component in `sources.rs` (new URL, new SHA-256 taken from the upstream release and checked by comparing with a second download), raise `RECIPE_VERSION` and `LIBMPV_RECIPE`, run the workflow, pin the new hashes.
- macOS packager: `--libmpv-dir DIR` bundles the libraries of `DIR/lib` and nothing from Homebrew, and puts `DIR/LICENSES` into `Contents/Resources/third-party`. Without it the packager refuses, unless `--allow-gpl-for-local-testing` is given (the old Homebrew path, local tests only).
- Ubuntu packager: `--libmpv-dir DIR` is required. The libraries go to `/usr/lib/video-all-in-one/`, the licence texts to `/usr/share/doc/video-all-in-one/third-party/`. The executable finds them through the run path `$ORIGIN/../lib/video-all-in-one`, which `release.yml` sets at build time through `RUSTFLAGS` (`-C link-arg=-Wl,-rpath,$ORIGIN/../lib/video-all-in-one`); the packager refuses an executable without it. `Depends:` is derived at packaging time from `readelf -d` of the executable and of every bundled library (`xtask/src/package/elf.rs` maps each soname to its Ubuntu package and fails on a soname it does not know); `libc6 (>= 2.35)` is the floor, so one package runs on 22.04 and 24.04.

## Bundled FFmpeg and FFprobe

The macOS and Ubuntu packages include FFmpeg and FFprobe from the same LGPL build as libmpv. They are installed in `Contents/Resources/bin` inside the macOS app and `/usr/lib/video-all-in-one/bin` on Ubuntu. The Windows installer is intended to install them in `<install folder>\bin`.

The libmpv recipe version is 2. Before a build can use these archives, the three archive hashes must be copied from the Build libmpv workflow into `LIBMPV_SHA256_LINUX_X86_64`, `LIBMPV_SHA256_MACOS_ARM64` and `LIBMPV_SHA256_MACOS_X86_64` in both `ci.yml` and `release.yml`. Those values are currently `PENDING-first-build`, so the workflows stop before building until the first archives are published and pinned.

The package licence scan refuses FFmpeg builds configured with `--enable-gpl`, `--enable-nonfree` or `--enable-libx264`.

Precise cut chooses an H.264 encoder offered by FFmpeg in this order: `libopenh264`, then `libx264`, then `h264_mf` on Windows, then `h264_videotoolbox` on macOS.

**NOT YET DONE:** Windows FFmpeg and FFprobe must come from the pinned LGPL build and be copied into a release of this repository before Windows packages can use them; see the Windows FFmpeg research note. The universal macOS DMG is also not yet done; the current packages have separate Apple silicon and Intel DMGs.

## App icons

The icons come from two SVG files: `assets/icons/app-icon.svg` (production) and `assets/icons/app-icon-dev.svg` (development, the same icon with an amber DEV band). To replace them, edit the SVGs (1024 x 1024, no text elements), run `cargo xtask icons` and commit the changed files in `assets/icons/generated/`. CI runs `cargo xtask icons --check` on Ubuntu and fails when the committed files differ from what the SVGs produce. The production packagers use `generated/production/` and the dev packagers (`--channel dev`) use `generated/development/`: `icon.icns` becomes `AppIcon.icns` in the macOS bundle, `icon.ico` is the icon of the Windows installer and uninstaller, and `icon.png` is installed in the `.deb` as the `hicolor` 512x512 icon. The window icon of a running app is not set yet (GPUI only supports it on X11 and needs the `image` crate for it).

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
| Runner images | `macos-15`, `macos-15-intel`, `windows-2025`, `ubuntu-22.04` (CI and release builds; `ubuntu-24.04` only for the release workflow's check and publish jobs) | Change the matrix when a runner is retired |
| libmpv for Windows | `MPV_WIN_URL` and `MPV_WIN_SHA256` in both workflow files; LGPL build of `zhongfly/mpv-winbuild` | Pick a newer release, copy the SHA-256 shown on its page, change both values in both files |
| libmpv and FFmpeg libraries for macOS and Ubuntu | Our own LGPL build: sources pinned in `xtask/src/libmpv/sources.rs`, archives pinned by `LIBMPV_SHA256_*` in both workflow files | See "Our own libmpv" |
| `ffmpeg` command in CI tests | Homebrew and apt, not pinned. A test tool only, never shipped | Whatever the runner installs |
| `yt-dlp` | Not bundled; it comes from the `PATH` | |

## Licences

- The Windows build links the LGPL build of libmpv.
- The macOS bundle and the Ubuntu package contain our own LGPL build of libmpv, FFmpeg and their libraries, with the licence texts and source URLs of every component (`LICENSES`). Users can replace the shared libraries. A notice file for the app itself that lists these components is not written yet.

### The license check of the packager

`cargo xtask package` refuses to build a macOS, Ubuntu or Windows package when a bundled library is a GPL or non-free build. It checks the FFmpeg configuration string inside every bundled library (`--enable-gpl`, `--enable-nonfree`) and the file names of the GPL codec libraries (`libx264`, `libx265`, `libxvid`, `libfdk-aac`, `libpostproc`). It does NOT check libmpv itself: libmpv has no marker the tool can read; that is why `cargo xtask libmpv build` builds it with `-Dgpl=false` and the recipe is tested. The Homebrew libraries are GPL builds, so the macOS packager only uses them behind the flag below. The flag `--allow-gpl-for-local-testing` lets a local run pass with a warning; a package made with it must never be published, and the release workflow must never pass it (a test in `xtask` fails if a workflow file contains it).

## Known limits

- macOS minimum version: our libmpv build is made for macOS 13 (`MACOSX_DEPLOYMENT_TARGET`), and `cargo xtask package` reads the real minimum from every file and writes the highest one into `LSMinimumSystemVersion`, so older systems get a clear refusal instead of a crash. Whether the build really runs on macOS 13 is not tested.
- Ubuntu: `Depends:` lists what `readelf -d` shows for the executable and the bundled libraries. Libraries that GPUI opens at run time with `dlopen` (Wayland, X11, Vulkan) do not appear there and are not declared; the CI step runs `ldd` to catch missing linked ones on the runner only.
- The `.deb` maintainer address is `noreply@example.invalid` until a real contact exists.

## What is unproven

Run on a Mac (Apple Silicon): `cargo xtask package --target macos` builds the app bundle, the zip and the dmg; the bundle starts and stays alive for six seconds with `DYLD_LIBRARY_PATH` empty; `otool -L` of the executable and of every bundled library shows no path under `/opt/homebrew` or `/usr/local`; the ad-hoc signature verifies.

Not run anywhere:

- The libmpv recipe: no build tool of it (`meson`, `ninja`) was installed on the machine that wrote it, so `cargo xtask libmpv build` was never run to the end, on any platform. Only the flags (checked against the option lists of the pinned archives), the hash table, the parsers, the pack step and the packagers' logic are unit tested. Everything about the real build is unproven until `build-libmpv.yml` runs: the meson option names and dependency fallbacks of mpv, libplacebo and HarfBuzz, whether mpv builds with GL, Vulkan and window systems disabled, the `patchelf` and `install_name_tool` relocation, the license scan on the result, the API probe, and the Ubuntu 22.04 build (glibc 2.35 floor, ALSA and PulseAudio from the system).
- The pinned archive hashes of the source table were computed from one download each over https (and match the sizes and names of the upstream releases), not checked against upstream signatures. The GitHub and GitLab archive URLs (mpv, libplacebo) are generated archives, so their hashes could change if the host changes how it compresses them.
- Both workflow files and `build-libmpv.yml` on GitHub, including downloading a release asset when the repository is private (the step sends no token).
- The macOS and Ubuntu packages made from our libmpv, their smoke tests, and the Ubuntu `.deb` install on 22.04 and 24.04.
- `ffmpeg` 4.4 from Ubuntu 22.04 apt as the test tool of the CI tests (the tests ran against newer FFmpeg builds before).
- Both workflow files on GitHub. They were checked as YAML only. Every step is unproven until a pull request and a release run.
- Intel macOS packaging (same code, other architecture).
- The Windows package: `makensis` was not available, so the generated NSIS script was unit tested as text only. The `lib.exe` step that makes `mpv.lib`, the assumption that the libmpv package contains `mpv.def` and `libmpv-2.dll`, linking the app against it, and the silent install and uninstall steps.
- The Ubuntu package: `dpkg-deb` was not available, so the control file and desktop entry were unit tested as text only. Building the `.deb`, installing it, the `ldd` check and removal.
- `cargo xtask icons --check` on Ubuntu compares bytes with files made on a Mac. If the renderer rounds differently on another CPU, the check fails there and the icons must be regenerated or the check relaxed.
- Fresh install, upgrade and uninstall on a real machine or clean VM for any OS, including the updater installing a package made by this pipeline.
- Notarization: not done, by design.
- `UPDATE_SIGNING_KEY`, `UPDATE_PUBLIC_KEY` and the tag ruleset are not set up; the key has not been generated.
