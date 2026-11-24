# Video All-in-One

Rust-native desktop app to download, cut, remove sections from, merge and export videos. Built with GPUI, FFmpeg and yt-dlp.

## Status

Early development. There is no release yet (the workspace version is 0.0.0). The features listed under "Planned features" are the plan, not what works today.

What works today:

- An empty window opens.
- The workspace has a language-share check, a structure check, build channels (development and production), an English and Vietnamese string table, a time parser, a job state machine and segment normalization. These are under test.

No download, cut or export feature works yet.

## Planned features

- Cut one or more segments from a local video.
- Download a whole video from a supported public URL.
- Download only chosen segments of an online video.
- Remove unwanted sections and join the rest.
- Save a project and continue later.
- Paste URLs with timestamps and let the app build the jobs.
- Pick segments on a timeline with preview.
- Export segments separately or merged.
- Process many sources in one go.
- A persistent job queue with progress, cancel and retry that survives restarts.

Also planned: the app updates itself from GitHub Releases, and the interface is in English and Vietnamese. The user picks the language at first launch and can change it in Settings.

## Supported platforms

- Windows 10 and later.
- A recent macOS. The packaged app carries its own libraries, so the minimum version is the one of the release's build machine and is stated in each release's notes.
- Ubuntu 22.04 and later.

macOS is the only platform exercised so far.

## Install

No installers exist yet. Releases will be published on the GitHub Releases page.

The project does not use paid code-signing certificates, so the first run of an unsigned build shows a Gatekeeper warning on macOS or a SmartScreen warning on Windows.

## Updates

The updater is planned as part of this project, written in Rust. It will check a signed manifest on GitHub Releases and verify the signature before installing anything. Development builds never contact the update endpoint.

## Build from source

You need:

- The stable Rust toolchain.
- FFmpeg and FFprobe and yt-dlp on the `PATH`. The app finds them there for now.
- On macOS, the Xcode command line tools with the Metal toolchain (GPUI needs it).

Commands that exist today:

```sh
cargo build --workspace
cargo run -p vaio-desktop
cargo test --workspace
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo xtask lang-share --min 85
cargo xtask check-structure
```

Production builds use:

```sh
cargo build --release -p vaio-desktop --features production
```

Do not use `--workspace --features production`.

## Project layout

The crate map is in `docs/architecture.md`.

| Path | Purpose |
|------|---------|
| `apps/desktop` | GPUI views; connects the interface to the core. |
| `crates/core` | Domain types, services and ports, without GPUI. |
| `crates/media` | Media processing and playback integrations. |
| `crates/storage` | Persistent storage with SQLite. |
| `crates/platform` | Operating system integration and external tool resolution. |
| `xtask` | Workspace maintenance and release commands (Rust). |

## Contributing

- At least 85 percent of the code is Rust. This is checked by `cargo xtask lang-share`.
- Crate dependency direction and project structure are checked by `cargo xtask check-structure`.
- Commits follow Conventional Commits: `type(scope): description`.
- Documents are in `docs/`. The design spec and plans are under `docs/superpowers/`. The sprint tracker is `docs/sprint-process.md`.

## License

Licensed under the MIT License. See the [LICENSE](LICENSE) file.

### Third-party components

Release packages bundle libmpv (the player) and FFmpeg libraries. Only LGPL builds of them are acceptable: the packaging command refuses to build a package that contains a GPL or non-free FFmpeg or libmpv (see `docs/RELEASING.md`). They remain under their own licenses, and their source code is available from their upstream projects. On Ubuntu the package depends on the system's libmpv instead of bundling it.

## Responsible use

This app is a tool. You are responsible for using it only on videos you own or are allowed to process, and for following the terms of the sites you download from.
