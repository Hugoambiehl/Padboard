# Protocol fixtures

Reference JSON for every message type, one file per `type` tag
(`:` replaced by `_`, e.g. `button:press` → `button_press.json`).

They are the cross-language contract: the **same files** are decoded by the Rust
tests (`crates/protocol/tests/fixtures.rs`) and by the Android unit tests. When a
message is added or changed in `crates/protocol`, add or update its fixture here.

`vectors/` holds test vectors that are not messages: `fingerprint.json` checks the
short TLS key fingerprint (`src/fingerprint.rs`) computed by both sides.

`core/` keeps the frozen core of every protocol version from 4 on
(`hello`, `welcome`, `update:*`: see the crate documentation), as
`<type>-v<N>.json`. They are never edited: when the protocol version is
bumped, copy the new version's core fixtures there. The Rust and Kotlin
tests check that they all still decode.
