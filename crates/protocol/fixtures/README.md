# Protocol fixtures

Reference JSON for every message type, one file per `type` tag
(`:` replaced by `_`, e.g. `button:press` → `button_press.json`).

They are the cross-language contract: the **same files** are decoded by the Rust
tests (`crates/protocol/tests/fixtures.rs`) and by the Android unit tests. When a
message is added or changed in `crates/protocol`, add or update its fixture here.
