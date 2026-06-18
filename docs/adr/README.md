# Architectural Decision Records

temper-git uses MADR (Markdown Architectural Decision Records).

- Write an ADR when a decision is viable-to-alternate, costly to reverse,
  or crosses components.
- Don't write one for implementation details — use an RFC instead.
- Format: `NNNN-short-title.md`, where NNNN is sequential.

## Accepted

- [0001-temper-git-mission.md](0001-temper-git-mission.md) — build a
  version-control experiment tailored for Dark Factories rather than
  adapting an existing tool.
- [0002-temper-native-version-control.md](0002-temper-native-version-control.md)
  — version-control state lives in IOA entities; protocol handlers are
  WASM integrations. No host-side Rust extensions.
- [0003-byte-exact-git-compat.md](0003-byte-exact-git-compat.md) —
  byte-exact git compatibility is a product guarantee, enforced by CI.
- [0009-genesis-only-app-install-and-restart-recovery.md](0009-genesis-only-app-install-and-restart-recovery.md)
  — Genesis is the app source of truth; installed app refs recover from target
  Temper instance storage on restart.
- [0010-agent-app-repair-and-version-evolution.md](0010-agent-app-repair-and-version-evolution.md)
  — agents repair apps by publishing Genesis versions; lineage remains for
  forks/imports.
- [0028-upload-pack-fuel-budget-for-agent-apps.md](0028-upload-pack-fuel-budget-for-agent-apps.md)
  — large agent app clones need a higher upload-pack WASM fuel budget while
  keeping receive-pack at the existing budget.

## Proposed

(none)

## Rejected

(none)

## Superseded

(none)
