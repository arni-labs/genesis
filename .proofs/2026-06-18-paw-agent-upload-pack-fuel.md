# Paw-agent upload-pack fuel proof - 2026-06-18

## Scope

This proof records the Genesis production unblock for
`temperpaw/paw-agent.git` clone/fetch during the Katagami curation publishing
E2E.

## Before

Production `he-upload-pack` used:

```text
MaxFuel=20000000000
TimeoutSecs=300
MaxMemory=536870912
MaxResponseBytes=134217728
```

The `temperpaw/paw-agent` clone stalled around 78 percent object receipt and
timed out locally after 120 seconds. Railway logs showed `git_upload_pack`
failing with `fuel exhausted` after the `walk_reachable_objects` phase reported
494 objects.

## Change

Genesis endpoint seed scripts now set:

```text
HttpEndpoints('he-upload-pack').MaxFuel=100000000000
```

The live row was patched to the same value. The first retry still behaved like
the old route budget, so Genesis was redeployed to reload the running route
limits.

Production endpoint readback after redeploy:

```text
HttpEndpoints('he-upload-pack').fields.MaxFuel=100000000000
TimeoutSecs=300
MaxMemory=536870912
MaxResponseBytes=134217728
processed_idempotency_keys.update-upload-pack-budget-v1=2
```

Railway redeploy:

```text
deployment=ac136d41-cf8c-4cde-8e42-e473884d58b9
status=SUCCESS
service=genesis
environment=production
reason=redeploy
imageDigest=sha256:219801deb056b52f23dd00176953d910b285cf22a042b83fad986a4f57015b85
```

Health check:

```text
GET /healthz -> 200
```

## Live clone proof

Command shape:

```text
GIT_TERMINAL_PROMPT=0 git \
  -c http.extraHeader='X-Tenant-Id: default' \
  -c protocol.version=0 \
  clone --progress \
  https://genesis-production-164d.up.railway.app/temperpaw/paw-agent.git \
  /tmp/genesis-paw-agent-clone-proof.byFglu/clone
```

Result:

```text
started_at=2026-06-18T03:47:18Z
finished_at=2026-06-18T03:47:31Z
clone_rc=0
receiving_objects=494/494
pack_size=10.39 MiB
git fsck --full=pass
HEAD=dc6a81fd65ebef9514fd7e91a6b4fae92477c2b7
branch=main
in-pack=494
packs=1
size-pack=10654
```

Deployment logs for the successful clone:

```text
Genesis git upload-pack phase complete
phase=emit_pack
owner=temperpaw
repo=paw-agent
count=494
bytes=10894808
duration_ms=4356

Genesis git upload-pack phase complete
phase=total
owner=temperpaw
repo=paw-agent
count=494
bytes=10894808
duration_ms=12100
```

No `fuel exhausted` error appeared during the successful post-redeploy clone.
