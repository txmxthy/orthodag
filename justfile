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

deny:
    cargo deny check

# what CI runs
ci: lint test
