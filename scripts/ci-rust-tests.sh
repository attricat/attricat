#!/usr/bin/env bash
# Runs one of several slices of the workspace's Rust tests, so CI can spread
# them across runners. Running the tests, not compiling them, takes most of
# the time, and nearly all of that is the API's database-backed integration
# suite.
#
# Every slice builds the workspace's tests once and runs the executables from
# that build. Calling cargo again with another package selection would resolve
# other features and recompile dozens of crates.
#
# Slice 1 runs every test executable except that suite's, plus the doctests,
# so it takes a smaller share of the suite. The suite's tests are dealt to
# the slices by name in a repeating pattern of SLICE_1_SHARE tests for slice 1
# to every SHARE for each other slice.
#
# Usage: scripts/ci-rust-tests.sh <slice> <slices>
set -euo pipefail

readonly SLICE_1_SHARE=2
readonly SHARE=3
readonly INTEGRATION_SUITE=integration
readonly INTEGRATION_PACKAGE_MANIFEST=apps/api/Cargo.toml

slice="${1:?usage: $0 <slice> <slices>}"
slices="${2:?usage: $0 <slice> <slices>}"
if ! [[ "$slice" =~ ^[1-9][0-9]*$ && "$slices" =~ ^[1-9][0-9]*$ ]] || ((slice > slices)); then
  echo "slice must be between 1 and the number of slices" >&2
  exit 2
fi
root="$(cd "$(dirname "$0")/.." && pwd)"

# One build for every slice. Each line is "<executable>\t<manifest>\t<name>".
executables=()
while IFS= read -r line; do
  executables+=("$line")
done < <(
  cargo test --locked --workspace --no-run --message-format=json-render-diagnostics |
    jq -r 'select(.reason == "compiler-artifact" and .profile.test and .executable != null)
      | [.executable, .manifest_path, .target.name] | @tsv'
)

# Runs a test executable the way cargo test does: from its package directory.
run_tests() {
  local executable="$1" manifest="$2"
  shift 2
  (cd "$(dirname "$manifest")" && CARGO_MANIFEST_DIR="$(dirname "$manifest")" "$executable" "$@")
}

integration_executable=""
integration_manifest=""
for line in "${executables[@]}"; do
  IFS=$'\t' read -r executable manifest name <<<"$line"
  if [[ "$name" == "$INTEGRATION_SUITE" && "$manifest" == "$root/$INTEGRATION_PACKAGE_MANIFEST" ]]; then
    integration_executable="$executable"
    integration_manifest="$manifest"
  elif ((slice == 1)); then
    echo "running ${manifest#"$root/"} $name"
    run_tests "$executable" "$manifest"
  fi
done
if [[ -z "$integration_executable" ]]; then
  echo "found no $INTEGRATION_SUITE test executable" >&2
  exit 1
fi
if ((slice == 1)); then
  cargo test --locked --workspace --doc
fi

# libtest lists each test as "<name>: test"; benchmarks are not listed so.
tests=()
while IFS= read -r name; do
  tests+=("$name")
done < <(run_tests "$integration_executable" "$integration_manifest" --list --format terse | sed -n 's/: test$//p' | sort)
if ((${#tests[@]} == 0)); then
  echo "found no integration tests to slice" >&2
  exit 1
fi

# Position p in the repeating pattern belongs to slice 1 while it is below
# SLICE_1_SHARE, then to the other slices in turns of SHARE.
pattern=$((SLICE_1_SHARE + (slices - 1) * SHARE))
mine=()
for ((index = 0; index < ${#tests[@]}; index++)); do
  position=$((index % pattern))
  if ((position < SLICE_1_SHARE)); then
    owner=1
  else
    owner=$((2 + (position - SLICE_1_SHARE) / SHARE))
  fi
  if ((owner == slice)); then
    mine+=("${tests[index]}")
  fi
done
echo "integration slice $slice of $slices: ${#mine[@]} of ${#tests[@]} tests"
run_tests "$integration_executable" "$integration_manifest" --exact "${mine[@]}"
