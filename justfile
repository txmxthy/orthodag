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

# how long each graph takes to lay out, worst last
#
# Release, because a debug build is an order of magnitude slower and says
# nothing about the real cost. The scores go to /dev/null: the timings are on
# stderr, so a caller can keep either half on its own.
bench *flags="--wide":
    @cargo run -q --release --features mermaid --example score -- {{flags}} > /dev/null

# the same, laid out for a terminal 80 columns across
#
# A different cost entirely: fitting walks a ladder of ever tighter styles and
# draws and scores every rung. Graph analysis and candidate ordering are shared
# because neither depends on the requested width. Anything with a window pays
# this one.
bench-fit width="80":
    @cargo run -q --release --features mermaid --example score -- --wide --fit {{width}} > /dev/null

# every graph on one page, to be looked at (does not open a browser)
#
# Release. Nothing here is being debugged and the page is four layouts of every
# graph, one of them fitted to a width — which walks a ladder and is a whole
# layout a rung. Debug turned four minutes of waiting into what is now twenty
# seconds.
gallery *flags="":
    cargo run -q --release --features mermaid --example gallery -- {{flags}}

# rewrite the accepted public-corpus quality baseline
baseline:
    @tmp=$(mktemp); trap 'rm -f "$tmp"' EXIT; { echo 'format=1'; cargo run -q --example score -- --record; } > "$tmp"; mv "$tmp" testdata/quality-baseline.txt
    @echo "wrote testdata/quality-baseline.txt"

deny:
    cargo deny check

# what CI runs
ci: lint test
