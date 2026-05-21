# Genesis

Genesis is the Temper-native app registry. It is built on temper-git: app
bundles are normal git commits, registry records are Temper entities, and
installs happen through the spec-owned `App.Install` action.

The primary user is an agent. The clean mental model is:

```text
agent writes app files -> git push to Genesis -> RegisterNewApp/PublishNewVersion -> install owner/app@hash
```

## What Works Now

- Git smart HTTP push, clone, fetch, and `ls-remote`.
- Repository object ingestion through the governed `Repository.IngestPack`
  composite action.
- Genesis registry entities for `App`, `Lineage`, `Closure`, and
  `AppInstallation`.
- Spec-owned app actions: `RegisterNewApp`, `PublishNewVersion`, `Fork`, and
  `Install`.
- Genesis UI app browsing, GitHub-like file browsing, lineage view, and
  copyable OData, CLI, TemperPaw-shaped, and clone commands.
- `temper install owner/app@hash --tenant ... --url ...` for installing a
  pinned Genesis app ref.
- Railway deployment with Postgres-backed state:
  <https://genesis-production-164d.up.railway.app/genesis/>
- The public Railway registry is seeded with the real TemperPaw, Katagami, and
  Deep Sci-Fi app bundles listed below; deleted smoke-test apps are hidden from
  the default UI.
- Pinned app installs materialize the app dependency closure from Genesis rows
  and repository objects, then recover after Railway redeploy from Postgres.

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
pinned `owner/app@hash` ref, Git clone URL, OData install path, CLI command,
and TemperPaw-shaped tool call.

## Agent Path: Publish An App

This is the current low-level path an agent can use today.

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

The Genesis UI also shows a TemperPaw-shaped command:

```text
install_app({"source":"genesis","app_ref":"owner/name@HASH","tenant":"target-tenant","url":"https://genesis.example"})
```

TemperPaw can call this same action from its install tool. A deployed
TemperPaw instance should set Genesis as its default app source and pass pinned
`owner/app@hash` refs instead of reading app bundles from GitHub, submodules,
symlinks, or local catalog directories.

## Local Run

Build the WASM modules and serve Temper with Genesis bootstrapped:

```bash
rustup target add wasm32-wasip1
cargo build -p git_upload_pack -p git_receive_pack -p scm_ingest_pack -p app_registry \
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

The current public Railway proof is:

- URL: <https://genesis-production-164d.up.railway.app/genesis/>
- Latest verified redeploy: `8db49199-0713-4e5a-a6d5-56580a321c27`
- Real app seed: 20 active installable app bundles, 913 reachable Git objects,
  and 84 field-overflow bodies persisted through the Postgres shadow store.
- Clone proof: `temperpaw/paw-agent`, `temperpaw/paw-patrol`,
  `katagami/katagami-commons`, and `katagami/katagami-curation` clone from the
  public Railway URL at their registered hashes.
- Install proof: `temperpaw/paw-patrol@7deb98f716e5c0e709bb7871642bdb35400cd04b`
  installed by OData and a TemperPaw-shaped request. The installed tenant
  exposed `Files`, `Agents`, `PatrolRequests`, and `Signals`, and a real
  `Signal` row was created and read back.
- Recovery proof: after the fresh Railway redeploy above, `paw-agent` still
  cloned at `65fbd22270e4bf7304de2d9b6895a465c332d602` and `paw-patrol`
  installed into `genesis-final-031238`; the installed
  `Signals('sig-final-031246')` row read back successfully.

The broader smoke proof publishes a tiny app, registers it, installs it through
OData, a TemperPaw-shaped request, and the CLI, creates an entity from the
installed app, restarts the service, and verifies recovery:

```bash
TEMPER_URL=https://genesis-production-164d.up.railway.app \
RUN_ID=200215 \
scripts/live-genesis-install-e2e-smoke.sh
```

The smoke-test app from that run was archived/deleted after verification so the
default registry view stays focused on real apps. Its archived ref was:

```text
genesis-e2e/tiny-notes-200215@21559ab9908e58109bd175672313b76baab54239
```

## Current Sharp Edges

- There is not yet a single `genesis publish` or `temper genesis publish`
  command.
- Push-to-create is not finished; first publish still needs repository/app
  registration.
- TemperPaw's old local catalog is not removed from the TemperPaw repository by
  these two PRs. Genesis now has the real app bundles and install surface; the
  TemperPaw deployment/config should switch its default source to Genesis.

## Read Next

- [`APP.md`](APP.md) - app-level summary.
- [`genesis-goal-tracker.html`](genesis-goal-tracker.html) - implementation and
  verification tracker.
- [`docs/rfc/0003-genesis-app-registry.md`](docs/rfc/0003-genesis-app-registry.md)
  - registry design.
- [`docs/rfc/0002-push-and-clone.md`](docs/rfc/0002-push-and-clone.md) - git
  push/clone design.
