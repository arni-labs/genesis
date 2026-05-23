# ADR-0010: Agents repair apps through Genesis versions

## Status

Accepted.

## Context

Genesis is the app source of truth for TemperPaw and other Temper instances.
Agents need a deterministic path when an installed app is wrong or incomplete:
they should change the app itself, publish the change, install the pinned
result, and verify the behavior. The registry also needs to make that evolution
legible without confusing ordinary updates with forks.

Two histories exist:

- Git commit/version history inside one app repository.
- Lineage sidecars between different app repositories.

Treating every app update as lineage would make normal repair work look like a
fork. Treating forks as ordinary versions would hide derivation and provenance.

## Decision

Normal app repair and update work stays on the same `App` and `Repository`.
Publishing a new version writes git objects, then `App.PublishNewVersion`
advances `LatestVersionHash`. The pinned ref `owner/name@hash` is the install
unit.

Genesis is git-native, so a normal update may push objects and advance
`refs/heads/main` before the registry action runs. `App.PublishNewVersion`
treats a ref that already points at the pushed hash as valid and updates
`App.LatestVersionHash`; it still rejects divergent refs.

`Lineage` remains reserved for app-to-app ancestry: forks, imports, and future
merge/graft relationships where a child app/repository records a parent
app/repository and parent commit.

Genesis UI must show both views separately:

- **Versions**: the commit chain for the selected app, with copyable install
  commands for each pinned commit.
- **Lineage**: parent/child app relationships from `Lineage` rows.

`App.Install` records the hash from the supplied pinned `AppRef`. Installing
`owner/name@oldhash` must record `oldhash`; it must not silently collapse to the
app's latest hash.

## Consequences

- Agents have one repair workflow:
  `search Genesis -> edit package -> publish/update -> install pinned ref -> verify`.
- Genesis can show app evolution as versions without overloading lineage.
- Historical installs remain reproducible because each install records the
  selected version hash.
- Future approval or governance UX must wrap this same pinned-ref install
  semantic instead of inventing a parallel app-install path.

## Verification

- UI tests cover the Versions tab and selected pinned install commands.
- `app_registry` tests cover `App.Install` preserving the hash from `AppRef`.
- Live smoke should publish or update a safe app, install the returned ref, then
  verify the app appears as a new version rather than a new lineage row.
