# orthodag task runner — `just` lists recipes

default:
    @just --list

fmt:
    cargo fmt --all

lint:
    cargo fmt --all --check
    cargo clippy --all-targets --all-features -- -D warnings

test:
    cargo test --all-features

# rewrite testdata/graphs from the fixtures in tests/common
corpus:
    cargo run -q --features mermaid --example corpus

# every fixture scored, worst first
score *flags="":
    cargo run -q --example score -- {{flags}}

# store today's numbers as the baseline the ratchet compares against
baseline:
    @mkdir -p target/quality
    cargo run -q --example score -- --record > target/quality/baseline.txt
    @echo "wrote target/quality/baseline.txt"

deny:
    cargo deny check

# what CI runs
ci: lint test
