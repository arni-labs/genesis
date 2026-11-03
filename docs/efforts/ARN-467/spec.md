# Genesis serves git and app bundles to agents

## What an agent can do

Clone and fetch any repository it is permitted to read. Push to a repository it
is permitted to write, authenticating with a GitToken presented as HTTP Basic
where the token is the username and the password is empty. Install an app from
Genesis by pinned ref (`owner/app@hash`), which reads that version's bundle.

Each of those is a single request an agent makes directly. None of them requires
a human to approve, mint, or relay anything at the time of the request.

## What the wire modules need in order to serve it

The git endpoints run as WASM guests. To answer a request, a guest must:

- **call back into its own kernel** — to resolve the presented token, to read
  refs and objects. It addresses the kernel by the kernel's loopback origin, and
  the `http_call` gate permits that origin for each module that makes the call.
- **read the `blob_endpoint` bootstrap secret** — the three git guests derive
  their OData base from it, so `access_secret` is permitted for exactly those
  modules and that secret.
- **read and write the git object cache** — `read_blob_object` and
  `write_blob_object` on the `git-objects/*` and `field-overflow/*` namespaces.
- **resolve the GitToken presented to it** — `read` and `list` on `GitToken`,
  permitted to the six protocol modules by name, not to agents generally.

Each permit names the modules that need it. None is a blanket grant.

## Protocol surface

Genesis advertises `side-band-64k thin-pack ofs-delta`. It does not advertise
`shallow` or partial clone, so `--depth` and `--filter=blob:none` are not
available against it and a client requesting them gets nothing rather than an
error worth trusting.

## Bundle limits

An app bundle is bounded at 4096 files, 16 MiB per file, 256 MiB total, with a
16 MiB canonical tree. A version exceeding these can be published but never
installed — that mismatch is ARN-492 and is not fixed here.
