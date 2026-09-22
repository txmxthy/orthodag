//! The numbers may not get worse.
//!
//! Two tests. One says which fixtures are inside the vocabulary and refuses to
//! let any of them fall out. The other compares every number against a stored
//! baseline and refuses a change that makes the drawing worse.
//!
//! The accepted baseline is committed with the public corpus. It is rewritten by
//! hand, by the person running the loop, after a change has been looked at and
//! accepted:
//!
//! ```text
//! just baseline
//! ```

#![allow(clippy::unwrap_used, clippy::panic)]

mod common;

use std::collections::BTreeMap;
use std::path::Path;

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

const BASELINE_FORMAT: &str = "format=1";
const BASELINE_PATH: &str = "testdata/quality-baseline.txt";
const FIELDS: [&str; 14] = [
    "bends_fwd",
    "bends_skip",
    "junctions",
    "overlaps",
    "cross",
    "blends",
    "mixed",
    "asym",
    "detour",
    "jogs",
    "ink",
    "width",
    "height",
    "total",
];
const SOFT: [&str; 3] = ["cross", "asym", "detour"];

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
    let scores = scores();
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(BASELINE_PATH);
    let baseline = baseline(&path).unwrap_or_else(|why| panic!("{why}"));
    let fixture_names: Vec<&str> = scores.keys().map(String::as_str).collect();
    let baseline_names: Vec<&str> = baseline.keys().map(String::as_str).collect();
    assert_eq!(
        baseline_names, fixture_names,
        "{BASELINE_PATH} does not describe exactly the public fixtures"
    );

    let score_fields: Vec<&str> = scores
        .values()
        .next()
        .unwrap_or_else(|| panic!("the public corpus is empty"))
        .fields()
        .into_iter()
        .map(|(field, _)| field)
        .collect();
    assert_eq!(
        score_fields, FIELDS,
        "{BASELINE_PATH} is incompatible with this Score schema"
    );

    let mut worse = Vec::new();
    let (mut soft_was, mut soft_now) = (0, 0);

    for (name, was) in &baseline {
        let now = scores
            .get(name)
            .unwrap_or_else(|| panic!("fixture sets were checked above"));
        // The soft tier is not stored on its own; it is what it is made of.
        soft_was += SOFT.iter().map(|field| was[*field]).sum::<i64>();
        soft_now += now.soft();

        for (field, value) in now.fields() {
            let before = was[field];
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

/// Reads and validates the accepted public-corpus scores.
fn baseline(path: &Path) -> Result<BTreeMap<String, BTreeMap<String, i64>>, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|why| format!("cannot read {}: {why}", path.display()))?;
    parse_baseline(&text).map_err(|why| format!("{}: {why}", path.display()))
}

fn parse_baseline(text: &str) -> Result<BTreeMap<String, BTreeMap<String, i64>>, String> {
    let mut lines = text
        .lines()
        .enumerate()
        .filter(|(_, line)| !line.trim().is_empty());
    let Some((_, format)) = lines.next() else {
        return Err("baseline is empty".to_owned());
    };
    if format != BASELINE_FORMAT {
        return Err(format!(
            "incompatible baseline format `{format}`; expected `{BASELINE_FORMAT}`"
        ));
    }
    let mut baseline = BTreeMap::new();
    for (at, line) in lines {
        let line_number = at + 1;
        let mut parts = line.split_whitespace();
        let Some(name) = parts.next() else {
            return Err(format!("line {line_number}: fixture name is missing"));
        };
        let mut fields = BTreeMap::new();
        for pair in parts {
            let (field, raw) = pair
                .split_once('=')
                .ok_or_else(|| format!("line {line_number}: malformed field `{pair}`"))?;
            if !FIELDS.contains(&field) {
                return Err(format!("line {line_number}: unknown field `{field}`"));
            }
            let value = raw
                .parse::<i64>()
                .map_err(|_| format!("line {line_number}: `{field}` is not an integer"))?;
            if fields.insert(field.to_owned(), value).is_some() {
                return Err(format!("line {line_number}: duplicate field `{field}`"));
            }
        }
        let missing: Vec<_> = FIELDS
            .iter()
            .filter(|field| !fields.contains_key(**field))
            .collect();
        if !missing.is_empty() {
            return Err(format!(
                "line {line_number}: `{name}` is missing fields: {}",
                missing.into_iter().copied().collect::<Vec<_>>().join(", ")
            ));
        }
        if baseline.insert(name.to_owned(), fields).is_some() {
            return Err(format!("line {line_number}: duplicate fixture `{name}`"));
        }
    }
    if baseline.is_empty() {
        return Err("baseline contains no fixtures".to_owned());
    }
    Ok(baseline)
}

#[test]
fn the_baseline_contract_is_strict() {
    let missing = Path::new(env!("CARGO_MANIFEST_DIR")).join("testdata/not-a-baseline.txt");
    assert!(baseline(&missing).is_err());
    assert!(parse_baseline("").is_err());
    assert!(parse_baseline("format=2\n").is_err());
    assert!(parse_baseline("format=1\nchain cross=nope\n").is_err());
    assert!(parse_baseline("format=1\nchain cross=0\n").is_err());
}
