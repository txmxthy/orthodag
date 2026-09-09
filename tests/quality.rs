//! The numbers may not get worse.
//!
//! Two tests. One says which fixtures are inside the vocabulary and refuses to
//! let any of them fall out. The other compares every number against a stored
//! baseline and refuses a change that makes the drawing worse.
//!
//! The baseline lives under `target/`, so it is not committed and not something
//! CI can drift into agreeing with. It is written by hand, by the person running
//! the loop, after a change has been looked at and accepted:
//!
//! ```text
//! cargo run --example score -- --record > target/quality/baseline.txt
//! ```
//!
//! With no baseline the ratchet skips. That is deliberate: it is a local tool
//! for a human working a loop, not a gate a stranger has to satisfy to build.

#![allow(clippy::unwrap_used, clippy::panic)]

mod common;

use std::collections::BTreeMap;
use std::path::PathBuf;

use orthodag::Score;

/// Fixtures whose drawings are known to be inside the vocabulary today.
///
/// Every fixture belongs here, and now every fixture is here. `ladder` was the
/// last holdout: its two untagged runs turned on each other's corner rows, and
/// the placement descent moved the column that made them do it. The list is the
/// ratchet — a fixture that reaches zero is added and never removed.
const CLEAN: &[&str] = &[
    "chain", "cycle", "diamond", "fan-in", "fan-out", "ladder", "skip", "two-tags", "wide",
];

fn scores() -> BTreeMap<String, Score> {
    common::fixtures()
        .into_iter()
        .map(|(name, g)| (name.to_owned(), orthodag::score(&g)))
        .collect()
}

#[test]
fn the_clean_fixtures_use_only_the_vocabulary() {
    let scores = scores();
    let mut broken = Vec::new();

    for name in CLEAN {
        let score = scores
            .get(*name)
            .unwrap_or_else(|| panic!("{name} is not a fixture"));
        if score.vocabulary() != [0; 4] {
            broken.push(format!("  {name}: {score}"));
        }
    }

    assert!(
        broken.is_empty(),
        "these fell out of the vocabulary:\n{}",
        broken.join("\n")
    );
}

#[test]
fn every_fixture_that_is_not_clean_is_named() {
    let known: Vec<String> = scores()
        .iter()
        .filter(|(_, s)| s.vocabulary() != [0; 4])
        .map(|(name, _)| name.clone())
        .collect();
    let expected: [&str; 0] = [];

    assert_eq!(
        known, expected,
        "the list of fixtures outside the vocabulary changed; if one was fixed, add it to CLEAN"
    );
}

#[test]
fn nothing_regresses_against_the_baseline() {
    let Some(baseline) = baseline() else {
        println!("no baseline at target/quality/baseline.txt, skipping the ratchet");
        return;
    };

    let scores = scores();
    let mut worse = Vec::new();
    let (mut soft_was, mut soft_now) = (0, 0);

    for (name, was) in &baseline {
        let Some(now) = scores.get(name) else {
            worse.push(format!(
                "  {name}: in the baseline and gone from the fixtures"
            ));
            continue;
        };
        // The soft tier is not stored on its own; it is what it is made of.
        soft_was += ["cross", "asym", "detour"]
            .iter()
            .filter_map(|field| was.get(*field))
            .sum::<i64>();
        soft_now += now.soft();

        for (field, value) in now.fields() {
            let Some(before) = was.get(field) else {
                continue;
            };
            // The vocabulary tier may never rise for any fixture, and zero is
            // absorbing. The soft tier may rise a little for one fixture as
            // long as the total across all of them falls.
            let slack = match field {
                "bends_fwd" | "bends_skip" | "junctions" | "overlaps" => 0,
                "width" => 4,
                "height" => 2,
                _ => 3,
            };
            if value > before + slack {
                worse.push(format!("  {name}.{field}: {before} -> {value}"));
            }
        }
    }

    assert!(
        soft_now <= soft_was,
        "the soft tier rose overall: {soft_was} -> {soft_now}\n{}",
        worse.join("\n")
    );
    assert!(worse.is_empty(), "these got worse:\n{}", worse.join("\n"));
}

/// Reads the stored numbers, if there are any.
fn baseline() -> Option<BTreeMap<String, BTreeMap<String, i64>>> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/quality/baseline.txt");
    let text = std::fs::read_to_string(path).ok()?;

    let mut baseline = BTreeMap::new();
    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        let mut parts = line.split_whitespace();
        let Some(name) = parts.next() else { continue };
        let fields: BTreeMap<String, i64> = parts
            .filter_map(|pair| {
                let (field, value) = pair.split_once('=')?;
                Some((field.to_owned(), value.parse().ok()?))
            })
            .collect();
        baseline.insert(name.to_owned(), fields);
    }
    Some(baseline)
}
