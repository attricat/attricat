#!/usr/bin/env bash
# Runs one of several slices of the workspace's Rust tests, so CI can spread
# them across runners. Running the tests, not compiling them, takes most of
# the time, and nearly all of that is the API's database-backed integration
# suite.
#
# Slice 1 runs every test target except that suite: the other crates' tests,
# the API's unit tests and the doctests. The suite's tests are then dealt to
# the slices in turn by name, so each slice runs every Nth one.
#
# Usage: scripts/ci-rust-tests.sh <slice> <slices>
set -euo pipefail

slice="${1:?usage: $0 <slice> <slices>}"
slices="${2:?usage: $0 <slice> <slices>}"
if ! [[ "$slice" =~ ^[1-9][0-9]*$ && "$slices" =~ ^[1-9][0-9]*$ ]] || ((slice > slices)); then
  echo "slice must be between 1 and the number of slices" >&2
  exit 2
fi
integration=(-p api --test integration)

# One build for every target the slices share.
cargo test --locked --workspace --no-run

if ((slice == 1)); then
  cargo test --locked --workspace --exclude api
  cargo test --locked -p api --lib --bins
  cargo test --locked -p api --doc
fi

# libtest lists each test as "<name>: test"; benchmarks are not listed so.
tests=()
while IFS= read -r name; do
  tests+=("$name")
done < <(
  cargo test --locked "${integration[@]}" -- --list --format terse 2>/dev/null |
    sed -n 's/: test$//p' | sort
)
if ((${#tests[@]} == 0)); then
  echo "found no integration tests to slice" >&2
  exit 1
fi
mine=()
for ((index = slice - 1; index < ${#tests[@]}; index += slices)); do
  mine+=("${tests[index]}")
done
echo "integration slice $slice of $slices: ${#mine[@]} of ${#tests[@]} tests"
cargo test --locked "${integration[@]}" -- --exact "${mine[@]}"
