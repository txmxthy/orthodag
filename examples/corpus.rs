//! Writes `testdata/graphs` from the fixtures.
//!
//! ```text
//! just corpus
//! ```
//!
//! The fixtures are built in code because that is the easiest place to read
//! them; they live on disk as Mermaid as well so anything that is not this
//! crate can score against the same graphs. `tests/roundtrip.rs` fails if the
//! two drift apart.

#[path = "../tests/common/mod.rs"]
mod common;

use std::path::PathBuf;

fn main() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("testdata/graphs");
    if let Err(why) = std::fs::create_dir_all(&dir) {
        println!("cannot write {}: {why}", dir.display());
        return;
    }

    for (name, g) in common::fixtures() {
        let path = dir.join(format!("{name}.mmd"));
        match std::fs::write(&path, orthodag::io::mermaid_out::to_mermaid(&g)) {
            Ok(()) => println!("wrote {}", path.display()),
            Err(why) => println!("cannot write {}: {why}", path.display()),
        }
    }
}
