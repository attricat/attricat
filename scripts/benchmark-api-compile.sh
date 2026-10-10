#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
target_dir="${CARGO_TARGET_DIR:-$root/target/compile-benchmark}"
mode="${1:-all}"
export CARGO_TARGET_DIR="$target_dir"
cd "$root"

measure() {
  local label="$1"
  shift
  printf '\n== %s ==\n' "$label"
  /usr/bin/time -p "$@"
}

case "$mode" in
  cold|all)
    if [[ "$target_dir" != "$root/target/compile-benchmark" ]]; then
      echo "cold mode only removes the dedicated default target: $root/target/compile-benchmark" >&2
      exit 2
    fi
    rm -rf "$target_dir"
    measure cold cargo check --locked -p api --all-targets
    ;;
  warm) ;;
  *) echo "usage: $0 [all|cold|warm]" >&2; exit 2 ;;
esac

measure warm cargo check --locked -p api --all-targets

for source in \
  crates/domain/src/model.rs \
  crates/repository/src/repository/record_search.rs \
  crates/http/src/http/record_reads.rs \
  crates/extension-runtime/src/extension_runtime.rs
do
  touch "$source"
  measure "incremental:$source" cargo check --locked -p api --all-targets
done

printf '\nArtifacts retained in %s for repeatable warm measurements.\n' "$target_dir"
