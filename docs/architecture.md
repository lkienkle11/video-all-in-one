# Architecture and placement

The workspace separates product policy from its implementations:

- `crates/core` owns domain types, services and ports, without GPUI.
- `crates/media` implements media operations, process execution and the player adapter.
- `crates/storage` implements persistence with SQLite.
- `crates/platform` implements platform paths, binary resolution and updates.
- `apps/desktop` contains GPUI views and connects them to the core.
- `xtask` contains Rust-based project and release tooling.

`vaio-core` must not depend on another workspace crate, a `gpui*` crate,
`rusqlite`, `reqwest`, or a process-spawning crate. Media, storage and
platform may depend on `vaio-core` but no other workspace crate. Desktop may
depend on any workspace crate; xtask may depend on core and platform only.

Place code by responsibility and feature, with one responsibility per file:

- A domain type such as a video segment belongs in core's domain module.
- A port such as a player interface belongs in core's ports module.
- An adapter such as an FFmpeg runner belongs in the media crate.
- A UI view such as the export screen belongs in the desktop app.
- A UI component such as a segment selector belongs alongside its desktop feature.
- A database migration belongs in the storage crate's migration module.
- A test belongs beside the behavior it verifies or in its crate's tests directory.
- A project script belongs in xtask as a Rust command.

Do not create source directories or Rust files named `utils`, `helpers`,
`common`, `misc`, `shared`, `manager`, or `stuff`. Keep source files at most
400 lines and directories at most 12 Rust source files.

How to add a directory: create it, run `cargo xtask structure-map --write`,
replace `TODO` with one sentence, commit the map with the code.

<!-- structure-map:start -->
apps/desktop | Presents the desktop interface and connects it to application services.
apps/desktop/src/ui | Presents language selection, Settings and update confirmation through the shared modal policy.
crates/core | Defines domain policy, services and ports independently of adapters.
crates/core/src/domain | Holds the core domain types: media timestamps, job identity, job kinds, job states and the job transition rules.
crates/core/src/i18n | Holds the languages, UI text keys and English and Vietnamese string tables.
crates/core/src/ports | Declares the traits the core needs from adapters, such as the update backend, the artifact verifier, the job store and the video player.
crates/core/src/services | Turns segments and a cut mode into the FFmpeg steps, support files and outputs of an export, without running any process.
crates/core/src/update | Parses and validates the update manifest and decides whether a newer version is offered, without I/O.
crates/media | Implements media processing and playback integrations.
crates/platform | Provides operating system integration and external tool resolution.
crates/platform/src/update | Fetches updates over HTTPS, stages and verifies artifacts, and installs or reveals them with per-OS commands and macOS bundle rollback.
crates/platform/src/update/steps | Holds the tested install building blocks: private verified file copies, the root-side Linux package step, macOS bundle path rules and update crash recovery.
crates/storage | Implements persistent storage for application data.
xtask | Provides workspace maintenance and release commands.
xtask/src/check_structure | Checks workspace dependencies, source placement, and the architecture map.
xtask/src/release | Release commands: versioning, update signing keys and signatures, and the update manifest.
<!-- structure-map:end -->
