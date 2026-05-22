#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

fail=0

if [[ -d canonical ]]; then
  printf 'error: root canonical/ directory must not exist; Git object logic belongs under crates/git_object\n' >&2
  fail=1
fi

if [[ -d wasm-modules ]]; then
  printf 'error: root wasm-modules/ directory must not exist; runtime integrations belong under wasm/<module> and helpers under crates/\n' >&2
  fail=1
fi

if rg -n '"canonical"|tg-canonical|tg_canonical' Cargo.toml registry temper/crates --glob '!target/**'; then
  printf 'error: native workspace/registry/temper crates still reference the old canonical crate\n' >&2
  fail=1
fi

if rg -n 'genesis-git-object|genesis_git_object' registry temper/crates --glob '!target/**'; then
  printf 'error: native registry/temper crates must not depend on genesis-git-object\n' >&2
  fail=1
fi

exit "$fail"
