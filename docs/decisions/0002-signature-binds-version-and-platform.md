# 0002: The update signature binds version, platform and size

Status: accepted.

Problem: the signature covered only the SHA-256 digest. Version, url and size came from the unsigned latest.json, so a hostile manifest could offer an old, genuinely signed artifact as version 99.0.0 (downgrade or replay), or one platform's file for another.

Decision: the signed message is the exact bytes
`vaio-update-v1\n<version>\n<platform_key>\n<size decimal>\n<sha256 lowercase hex>\n`.
One function builds it: `vaio_core::update::signed_message` (also `UpdateClaim::signed_message`).

- The app builds the claim from the offered version, the platform key it looked up, the manifest size and the digest computed while downloading. It still compares the digest with the manifest hash and the downloaded size with the manifest size.
- `cargo xtask sign` takes version and platform key from the file name `video-all-in-one-<version>-<platform_key>.<ext>`, and size and digest from the file.
- `cargo xtask latest-json` verifies every signature with the same function before it writes the manifest.
- Ed25519 `verify_strict` checks the signature. Manifest schema 1 keeps its field names; `signature` now means this message.

Why: a manifest can only select among artifacts the key holder signed for that exact version, platform and size. The `vaio-update-v1` prefix separates this message from any other signed data. Old signatures over the bare digest no longer verify, so artifacts must be signed again.
