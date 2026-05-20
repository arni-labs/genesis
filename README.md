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

TemperPaw itself is not fully migrated to this Genesis tool path yet.

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

The current E2E proof publishes a real app, registers it, installs it through
OData, a TemperPaw-shaped request, and the CLI, creates an entity from the
installed app, restarts the service, and verifies recovery:

```bash
TEMPER_URL=https://genesis-production-164d.up.railway.app \
RUN_ID=200215 \
scripts/live-genesis-install-e2e-smoke.sh
```

Verified app ref:

```text
genesis-e2e/tiny-notes-200215@21559ab9908e58109bd175672313b76baab54239
```

## Current Sharp Edges

- There is not yet a single `genesis publish` or `temper genesis publish`
  command.
- Push-to-create is not finished; first publish still needs repository/app
  registration.
- Fresh deployments may need smart HTTP endpoints seeded before the first git
  push; the live E2E script does this.
- TemperPaw still installs from its local app catalog today. The Genesis-shaped
  install path is implemented and verified, but TemperPaw needs a follow-up
  migration to use it by default.

## Read Next

- [`APP.md`](APP.md) - app-level summary.
- [`genesis-goal-tracker.html`](genesis-goal-tracker.html) - implementation and
  verification tracker.
- [`docs/rfc/0003-genesis-app-registry.md`](docs/rfc/0003-genesis-app-registry.md)
  - registry design.
- [`docs/rfc/0002-push-and-clone.md`](docs/rfc/0002-push-and-clone.md) - git
  push/clone design.
