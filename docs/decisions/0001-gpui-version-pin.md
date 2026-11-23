# 0001: Pin GPUI Kit and its GPUI snapshot

Status: accepted.

## Decision

The workspace pins `gpui-kit = "=0.11.0"`. The desktop application inherits
that dependency for production and enables `test-support` on the same workspace
dependency for tests. It imports GPUI through `gpui_kit` only.

GPUI Kit owns the matching GPUI snapshot. Its published manifest aliases
`gpui-pre` as `gpui` and pins it, `gpui-pre-macros`, `gpui-pre-platform`, and
`gpui-pre-web` to `=0.3.14`. Do not add a second GPUI source, direct GPUI
dependency, or patch: a different snapshot can introduce incompatible types
and platform implementations.

## Resolved versions

These versions were read from this workspace's `Cargo.lock`; the alias `gpui`
does not appear as a package named `gpui` in the lockfile.

| Package or family | Locked version | Role |
| --- | --- | --- |
| `gpui-kit` | 0.11.0 | Application facade |
| `gpui-base` | 0.11.0 | Reusable behavior and window Root |
| `gpui-component`, `gpui-component-macros` | 0.11.0 | Styled components and derives |
| `gpui-kit-assets` | 0.11.0 | Default assets |
| `gpui-pre` (dependency alias `gpui`) | 0.3.14 | Default GPUI engine |
| `gpui-pre-macros`, `gpui-pre-scheduler` | 0.3.14 | Engine macros and scheduling |
| `gpui-pre-platform` | 0.3.14 | Default platform selection |
| `gpui-pre-apple`, `gpui-pre-macos`, `gpui-pre-ios` | 0.3.14 | Apple platform support |
| `gpui-pre-linux`, `gpui-pre-windows` | 0.3.14 | Linux and Windows backends |
| `gpui-pre-web`, `gpui-pre-wgpu` | 0.3.14 | Web and WGPU backends |
| `gpui-pre-bench-metrics`, `gpui-pre-collections`, `gpui-pre-derive-refineable`, `gpui-pre-http-client`, `gpui-pre-refineable`, `gpui-pre-shared-string`, `gpui-pre-sum-tree`, `gpui-pre-util`, `gpui-pre-ztracing`, `gpui-pre-ztracing-macro` | 0.3.14 | Snapshot support crates |
| `gpui-pre-reqwest` | 0.12.15 | Snapshot HTTP implementation |
| `gpui-fast`, `gpui-fast-macros`, `gpui-fast-scheduler` | 0.1.19 | Optional Fast engine family |
| `gpui-fast-platform`, `gpui-fast-apple`, `gpui-fast-macos`, `gpui-fast-linux`, `gpui-fast-windows`, `gpui-fast-web`, `gpui-fast-wgpu` | 0.1.19 | Optional Fast platform family |

The Fast packages are present in the lockfile because Kit offers an optional
`gpui-fast` feature. The desktop application does not enable that feature;
their presence is not an application-added second GPUI source. Platform entries
in the lockfile describe resolution for all targets, not runtime verification
on each operating system.

## Upgrade procedure

Change the single exact GPUI Kit version deliberately, let Cargo resolve its
matching snapshot, and review the manifest and lockfile changes together.
Keep production and test dependencies on the same workspace pin. Do not upgrade
individual GPUI snapshot crates independently.

Run `cargo test -p vaio-desktop window_opens_with_app_view`,
`cargo test --workspace`, `cargo fmt --check`,
`cargo clippy --workspace --all-targets -- -D warnings`, and
`cargo xtask check-structure`. Then run the desktop application manually on the
supported platforms to verify native window behavior; the headless smoke test
does not prove display or packaging behavior.
