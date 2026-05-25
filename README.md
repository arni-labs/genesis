# Genesis

Genesis is the Temper-native app registry. It is built on temper-git: app
bundles are normal git commits, registry records are Temper entities, and
installs happen through the spec-owned `App.Install` action.

The primary user is an agent. The clean mental model is:

```text
agent fixes app files -> publish/update in Genesis -> install owner/app@hash -> verify
```

## What Works Now

- Git smart HTTP push, clone, fetch, and `ls-remote`.
- Repository object ingestion through the governed `Repository.IngestPack`
  composite action.
- Genesis registry entities for `App`, `Lineage`, `Closure`, and
  `AppInstallation`.
- Spec-owned app actions: `RegisterNewApp`, `PublishNewVersion`, `Fork`, and
  `Install`.
- Genesis UI app browsing, GitHub-like file browsing, version evolution,
  lineage view, and copyable OData, CLI, TemperPaw tool, and clone commands.
- `temper install owner/app@hash --tenant ... --url ...` for installing a
  pinned Genesis app ref.
- Vercel-hosted Genesis UI:
  <https://genesis-registry-ui.vercel.app>
- Railway deployment with Postgres-backed API/git state:
  <https://genesis-production-164d.up.railway.app>
- The public Railway registry is seeded with the real TemperPaw, Katagami, and
  Deep Sci-Fi app bundles listed below; deleted smoke-test apps are hidden from
  the default UI.
- Pinned app installs materialize the app dependency closure from Genesis rows
  and repository objects, then recover after Railway redeploy from Postgres.
- Git clone performance now uses a raw object-body cache populated on ingest
  and warmed on upload-pack fallback; install performance is measured
  separately from clone performance.
- `PublishNewVersion` requires the new app hash to be an existing Git commit in
  the app repository, so future app refs stay installable and clonable.

## Seeded Railway Apps

The current public Genesis registry contains:

- TemperPaw core apps: `paw-fs`, `paw-agent`, `paw-research`,
  `paw-channels`, `paw-compute`, `paw-ingest`, `paw-pm`, `paw-harness`,
  `paw-heal`, `paw-managed-agents`, `paw-wiki`, `paw-foresight`,
  `paw-consilium`, `paw-autoreason`, and `paw-skills`.
- Katagami apps from the canonical Katagami workspace: `katagami-commons` and
  `katagami-curation`.
- Deep Sci-Fi reference apps: `dsf-harness` and `dsf-team`.

Each app is stored as normal Genesis repository objects. The UI exposes the
pinned `owner/app@hash` ref, version chain, Git clone URL, OData install path,
CLI command, and TemperPaw tool call.

## Agent Path: Publish Or Update An App

The primary agent surface is:

```python
ref = temper.publish_app({
    "path": "/workspace/my-app",
    "owner": "owner",
    "name": "name",
    "registry_url": GENESIS_URL,
    "message": "Publish my app"
})
```

`temper.update_app(...)` uses the same shape and returns a new pinned
`owner/name@hash` ref. Internally these tools use Genesis' git transport; agents
should not hand-roll git and curl as the normal workflow.

When an installed app is wrong, broken, or missing a sensor/capability, the
agent should update the app package itself and publish the next Genesis version.
That is a normal version update: the same `App` and `Repository` advance to a
new commit hash through `PublishNewVersion`. `Lineage` is reserved for forks,
imports, and derivatives where a child app/repository points back to a parent.
The hash in `owner/name@hash` is the Git commit hash for the app version.

The current low-level/admin path is:

1. Create or choose an app bundle directory. A bundle may contain:

   ```text
   app.toml
   specs/
   policies/
   wasm/
   content/
   agents/
   agent-skills/
   adrs/
   seed-data/
   ```

   Runtime WASM integrations live under `wasm/<module>/`; Cargo build
   output stays in `target/`, and Temper loads the packaged
   `wasm/<module>/<module>.wasm` artifact. Shared Rust helper crates, if
   present, belong under `crates/`.

2. Create the Genesis repository row:

   ```bash
   curl -sS -X POST "$GENESIS_URL/tdata/Repositories" \
     -H "Content-Type: application/json" \
     -H "X-Tenant-Id: default" \
     -d '{"Id":"rp-owner-name","OwnerAccountId":"owner","Name":"name","Description":"App repo","DefaultBranch":"main","Visibility":"public"}'
   ```

3. Wait for the repository to become `Active`. `Repository.Create` is meant to
   fire the provisioning integration; the repo must be active before git push.
   The live smoke script includes the current explicit provisioning callback
   used by the test harness.

4. Push the app bytes with normal git:

   ```bash
   git init -b main
   git add .
   git commit -m "Publish app"
   git push "$GENESIS_URL/owner/name.git" main
   HASH="$(git rev-parse HEAD)"
   ```

   For large app bundles, use Git's non-chunked request mode until streaming
   request chunk support is added:

   ```bash
   git -c http.postBuffer=104857600 push "$GENESIS_URL/owner/name.git" main
   ```

5. Register the app row:

   ```bash
   curl -sS -X POST "$GENESIS_URL/tdata/Apps('app-owner-name')/Temper.Git.RegisterNewApp?await_integration=true" \
     -H "Content-Type: application/json" \
     -H "X-Tenant-Id: default" \
     -d '{"Name":"name","RepositoryId":"rp-owner-name","Description":"App repo","Exports":"{}","Visibility":"public"}'
   ```

6. The pinned app ref is now:

   ```text
   owner/name@HASH
   ```

For a full executable proof, see
[`scripts/live-genesis-install-e2e-smoke.sh`](scripts/live-genesis-install-e2e-smoke.sh).
For read-only clone performance proof, see
[`scripts/live-genesis-clone-performance-smoke.sh`](scripts/live-genesis-clone-performance-smoke.sh).

## Agent Path: Install An App

Install through the spec-owned OData action:

```bash
curl -sS -X POST "$GENESIS_URL/tdata/Apps('app-owner-name')/App.Install?await_integration=true" \
  -H "Content-Type: application/json" \
  -H "X-Tenant-Id: default" \
  -d '{"TargetTenant":"target-tenant","AppRef":"owner/name@HASH","Installer":"agent"}'
```

Or use the CLI wrapper:

```bash
temper install owner/name@HASH --tenant target-tenant --url "$GENESIS_URL"
```

The Genesis UI also shows the TemperPaw tool call:

```text
temper.install_app({"app_ref":"owner/name@HASH","tenant":"target-tenant","registry_url":"https://genesis.example"})
```

TemperPaw calls the local Temper Genesis installer, which materializes the
pinned closure from Genesis into that Temper instance and records durable
installed-app provenance. Runtime install fetches the pinned Genesis bundle;
`git clone` remains the authoring/inspection path, not the normal install
primitive. Use `temper.search_apps(...)` to discover app refs.

The UI's Versions tab shows install commands for each pinned commit. Installing
an older pinned ref records that selected version hash; it does not silently
install the latest version.

## Local Run

Build the WASM modules and serve Temper with Genesis bootstrapped:

```bash
rustup target add wasm32-wasip1
cargo build -p git_refs_advertise -p git_upload_pack -p git_receive_pack -p scm_ingest_pack -p app_registry \
  --target wasm32-wasip1 --release

TEMPER_OS_APPS_DIR="$PWD" cargo run \
  --manifest-path temper/Cargo.toml \
  --release --bin temper \
  -- serve --port 3000 --storage turso --app temper-git
```

Then open:

```text
http://127.0.0.1:3000/genesis/
```

## Live Verification

The current public proof is:

- Vercel UI URL: <https://genesis-registry-ui.vercel.app>
- Railway API/git URL: <https://genesis-production-164d.up.railway.app>
- Latest verified Railway deploy: `a494ba4c-429c-4cec-bb88-5e4965c5a9c5`
- Vercel production deploy:
  <https://genesis-registry-dwvt6o7gs-rita-agafonovas-projects.vercel.app>
  aliased to <https://genesis-registry-ui.vercel.app>
- Real app seed: 20 normal active installable app bundles plus the bootstrap
  `temper-git` platform app, with smoke-test apps marked `Deleted` so the
  default registry view stays focused on real apps.
- UI proof: the Vercel page loaded `paw-agent`, `katagami-curation`, and
  `paw-patrol` from the Railway API, opened the `paw-patrol` file browser, and
  showed OData, CLI, TemperPaw tool, and clone commands pointing at the
  Railway registry URL.
- Clone proof: valid production refs such as `temperpaw/paw-patrol`,
  `katagami/katagami-commons`, and `katagami/katagami-curation` clone from the
  public Railway URL at their registered hashes. A historical `paw-agent` row
  currently points at a non-commit app hash and must be repaired through a
  governed Genesis update before it is used as a clone-performance target.
- Install proof: `temperpaw/paw-patrol@7deb98f716e5c0e709bb7871642bdb35400cd04b`
  installed by OData and the TemperPaw tool-shaped request. The installed tenant
  exposed `Files`, `Agents`, `PatrolRequests`, and `Signals`, and a real
  `Signal` row was created and read back.
- Recovery proof: after the fresh Railway redeploy above, `paw-patrol`
  installed into `genesis-final-031238`; the installed
  `Signals('sig-final-031246')` row read back successfully.

The broader smoke proof publishes a tiny app, registers it, installs it through
OData, the TemperPaw-shaped path, and the CLI, creates an entity from the
installed app, restarts the service, and verifies recovery:

```bash
TEMPER_URL=https://genesis-production-164d.up.railway.app \
RUN_ID=rail133900 \
scripts/live-genesis-install-e2e-smoke.sh
```

The smoke-test app from that run was archived/deleted after verification so the
default registry view stays focused on real apps. Its archived ref was:

```text
genesis-e2e/tiny-notes-rail133900@8ff05405d769eccbeeb7cab3b15cf96dc269abb8
```

## Current Sharp Edges

- There is not yet a single `genesis publish` or `temper genesis publish`
  command.
- Push-to-create is not finished; first publish still needs repository/app
  registration.
- Genesis does not yet build WASM artifacts on publish. Today app bundles must
  include packaged `wasm/<module>/<module>.wasm` files; Temper install/reconcile
  hashes those existing bytes, persists them, and caches them for execution.
  Roadmap: build and verify WASM artifacts once during Genesis publish with a
  pinned toolchain, store them with the app ref, and keep install deterministic.
- TemperPaw still keeps local app directories for development and test fixtures,
  but the normal agent-facing install/search/publish/update path is Genesis.
  Fresh production bootstrap should be configured with pinned Genesis refs; warm
  restart recovers already-installed app state from the Temper instance DB.

## Read Next

- [`APP.md`](APP.md) - app-level summary.
- [`docs/adr/0009-genesis-only-app-install-and-restart-recovery.md`](docs/adr/0009-genesis-only-app-install-and-restart-recovery.md)
  - Genesis-only app install and restart recovery decision.
- [`docs/adr/0010-agent-app-repair-and-version-evolution.md`](docs/adr/0010-agent-app-repair-and-version-evolution.md)
  - agent-first app repair and version-vs-lineage semantics.
- [`docs/rfc/0003-genesis-app-registry.md`](docs/rfc/0003-genesis-app-registry.md)
  - registry design.
- [`docs/rfc/0002-push-and-clone.md`](docs/rfc/0002-push-and-clone.md) - git
  push/clone design.
