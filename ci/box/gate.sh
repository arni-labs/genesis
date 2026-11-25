#!/usr/bin/env bash
# Runs on ci-genesis, as tl-user: tests one commit as .github/workflows/ci.yml did on GitHub's runners for a push to
# main (jobs rust, web, workflow-gate, by the same steps and the commit's own scripts), and builds the commit's
# Railway image from its Dockerfile with a warm BuildKit cache, tagged ghcr.io/arni-labs/genesis:sha-<7>:
#   gate.sh <commit> <base>
# ci/box/box.sh uploads this file and the git bundle (/var/lib/ci/in.bundle) before it calls it. Warm by design: one
# worktree at one path, so a checkout keeps unchanged files' times and cargo rebuilds only what the commit changed; each
# target/ and web/node_modules survive the clean; the kernel the submodule pins is built once per pin. The temper/
# submodule comes from GitHub (arni-labs/temper is public) into a clone the box keeps. Docker runs for the gate's
# length only; its build cache stays on disk. Answers go to /var/lib/ci/runs/<commit>/ (<job>.txt ending `exit <code>`;
# summary.txt; image-name.txt naming the built image; verdict.txt last).
# Left out on purpose: pushing the image (ci/box/publish.sh does that, with a credential the box does not keep).
# Exit 0 when every job passed, 1 when one failed, 2 on a usage error. Prints no key.
set -uo pipefail
# The Factory's command holds /var/lib/ci/box.lock on fd 9 for as long as this runs. Closed here, so no daemon this
# starts (a server, Postgres, Redis, dockerd) inherits it and keeps the box locked after a run that was killed.
exec 9>&-
commit=${1:?usage: gate.sh <commit> <base>} base=${2:?usage: gate.sh <commit> <base>}
CI=/var/lib/ci repo=/var/lib/ci/repo.git tree=/var/lib/ci/tree temper_repo=/var/lib/ci/temper.git
. /opt/ci/env.sh
[ "$(id -u)" != 0 ] || { echo "gate: run as a user that is not root, as GitHub's runner is"; exit 2; }
t0=$SECONDS
if [ -s "$CI/in.bundle" ]; then
    git -C "$repo" fetch -q "$CI/in.bundle" '+refs/ci/*:refs/ci/*' || { echo "gate: the bundle did not fetch"; exit 2; }
    rm -f "$CI/in.bundle"
fi
# A commit the box lacks comes from origin (https://github.com/arni-labs/genesis.git), as the Factory's route runs a gate: no bundle, only the
# two commits. ci/box/box.sh's bundle carries them instead when it drives the box.
keys=${CI_KEYS:-$CI/keys}
git -C "$repo" remote get-url origin >/dev/null 2>&1 || git -C "$repo" remote add origin https://github.com/arni-labs/genesis.git
for c in "$commit" "$base"; do
    git -C "$repo" cat-file -e "$c^{commit}" 2>/dev/null || git -C "$repo" fetch -q --depth=200 origin "$c" \
        || { echo "gate: $c is neither on the box nor fetched from origin"; exit 2; }
done
commit=$(git -C "$repo" rev-parse --verify "$commit^{commit}") || exit 2
base=$(git -C "$repo" rev-parse --verify "$base^{commit}") || exit 2
git -C "$repo" update-ref "refs/ci/tested/$commit" "$commit"
git -C "$repo" for-each-ref --sort=-committerdate --format='%(refname)' refs/ci/tested | tail -n +51 | xargs -r -n1 git -C "$repo" update-ref -d
if git -C "$tree" rev-parse --git-dir >/dev/null 2>&1; then
    git -C "$tree" reset -q --hard && git -C "$tree" checkout -q --force --detach "$commit" \
      && git -C "$tree" clean -q -ffdx -e target/ -e node_modules/ -e /temper/ || exit 2
else
    rm -rf "$tree"; git -C "$repo" worktree prune; git -C "$repo" worktree add -q --force --detach "$tree" "$commit" || exit 2
fi
# The submodule at the commit's pin (checkout: submodules: recursive).
pin=$(git -C "$tree" ls-tree "$commit" temper | awk '$2 == "commit" {print $3}')
[ -n "$pin" ] || { echo "gate: $commit pins no temper submodule"; exit 2; }
[ -d "$temper_repo" ] || git init -q --bare "$temper_repo"
git -C "$temper_repo" cat-file -e "$pin^{commit}" 2>/dev/null \
  || git -C "$temper_repo" fetch -q --depth 1 https://github.com/arni-labs/temper "$pin" || { echo "gate: temper $pin did not fetch"; exit 2; }
git -C "$temper_repo" update-ref "refs/pins/$pin" "$pin"
if [ ! -e "$tree/temper/.git" ]; then rm -rf "$tree/temper"; git clone -q --shared --no-checkout "$temper_repo" "$tree/temper" || exit 2; fi
git -C "$tree/temper" fetch -q origin "refs/pins/$pin" && git -C "$tree/temper" checkout -q --force --detach "$pin" \
  && git -C "$tree/temper" clean -q -ffdx -e target/ || exit 2
out=$CI/runs/$commit && rm -rf "$out" && mkdir -p "$out" && touch "$out/summary.txt"
ulimit -n "$(ulimit -Hn)"
cd "$tree" || exit 2
exec 3>&1
echo "gate: $commit against $base on $(nproc) cores, temper at ${pin:0:12}; checkout in $((SECONDS - t0)) s"

# --- the jobs: one function each, named as ci.yml names them --------------------------------------------------------
job_rust() { # Rust Workspace
    echo "== cargo fmt"; cargo +"$RUST_NIGHTLY" fmt --all -- --check || return
    echo "== cargo test"; cargo +"$RUST_NIGHTLY" test --workspace --locked
}
job_web() { # Genesis Web, with CI set as on GitHub (playwright reads it)
    export CI=true; cd web || return
    local sum; sum=$(sha256sum package-lock.json | cut -c1-64)
    [ "$(cat node_modules/.ci-lock 2>/dev/null)" = "$sum" ] || { npm ci --no-audit --no-fund && echo "$sum" >node_modules/.ci-lock; } || return
    local pw; pw=$(node -p 'require("@playwright/test/package.json").version')
    if [ "$(cat "$PLAYWRIGHT_BROWSERS_PATH/.ci-deps" 2>/dev/null)" != "$pw" ]; then
        npx playwright install --with-deps chromium && echo "$pw" >"$PLAYWRIGHT_BROWSERS_PATH/.ci-deps" || return
    else npx playwright install chromium || return; fi
    echo "== svelte check"; npm run check || return
    echo "== build"; npm run build || return
    echo "== e2e"; npm run test:e2e
}
job_workflow_gate() { # GitHub Workflow Gate, on $RUST_STABLE as there, in targets of its own
    local modules="git_refs_advertise git_upload_pack git_receive_pack scm_ingest_pack scm_assign_pr_number scm_merge_pr app_registry github_rest_repos github_rest_refs github_rest_pulls"
    local m args=""; for m in $modules; do args="$args -p $m"; done
    echo "== wasm integrations"
    CARGO_TARGET_DIR=$tree/target/workflow-gate cargo +"$RUST_STABLE" build --release --target wasm32-wasip1 $args || return
    for m in $modules; do cp "target/workflow-gate/wasm32-wasip1/release/$m.wasm" "wasm/$m/$m.wasm" || return; done
    local kernel=$CI/kernels/$pin/temper
    if [ ! -x "$kernel" ]; then
        echo "== Temper kernel at ${pin:0:12}"
        CARGO_TARGET_DIR=$CI/kernel-target cargo +"$RUST_STABLE" build --manifest-path temper/Cargo.toml --release --bin temper || return
        mkdir -p "$(dirname "$kernel")" && cp "$CI/kernel-target/release/temper" "$kernel" || return
    else echo "== Temper kernel at ${pin:0:12}: built before"; fi
    local tmp port=3188 pid; tmp=$(mktemp -d)
    mkdir -p "$tmp/os-apps" && ln -sfn "$PWD" "$tmp/os-apps/temper-git"
    export TEMPER_API_KEY; TEMPER_API_KEY=$(openssl rand -hex 32)
    TEMPER_OS_APPS_DIR="$tmp/os-apps" TURSO_URL="file:$tmp/genesis-workflow-gate.db" \
      "$kernel" serve --port $port --storage turso --no-observe --app temper-git >"$tmp/server.log" 2>&1 &
    pid=$!
    local deadline=$((SECONDS + 300))
    until curl -fsS "http://127.0.0.1:$port/healthz" >/dev/null 2>&1; do
        kill -0 $pid 2>/dev/null || { echo "Genesis server exited during boot"; tail -200 "$tmp/server.log"; return 1; }
        [ $SECONDS -lt $deadline ] || { echo "Genesis server not ready within 300s"; tail -200 "$tmp/server.log"; kill $pid; return 1; }
        sleep 2
    done
    local c=0
    echo "== GitHub workflow round-trip smoke"; TEMPER_URL=http://127.0.0.1:$port scripts/live-github-workflow-smoke.sh || c=1
    if [ $c = 0 ]; then echo "== Private-read smoke"; TEMPER_URL=http://127.0.0.1:$port scripts/live-genesis-private-read-smoke.sh || c=1; fi
    [ $c = 0 ] || tail -300 "$tmp/server.log"
    kill $pid 2>/dev/null; wait $pid 2>/dev/null
    return $c
}
# The image Railway built from the Dockerfile on every push (skipBuildCache, 15-24 min there). The checkout holds the
# submodule, so the Dockerfile's fetch of it is skipped, as it says for CI. Tags: sha-<7> (what the Factory's railway
# target names) and the full commit.
# The image builds from the commit's files alone (and the temper/ its submodule pins), as Railway's snapshot was, not
# from the shared tree: the other jobs leave files there (wasm/ copies, temper/.git), which changed `COPY . .` and so
# every layer after it on each run.
job_image() {
    local image=ghcr.io/arni-labs/genesis ctx=$CI/image-ctx
    rm -rf "$ctx" && mkdir -p "$ctx/temper" && git -C "$repo" archive "$commit" | tar -x -C "$ctx" \
      && git -C "$temper_repo" archive "$pin" | tar -x -C "$ctx/temper" && cd "$ctx" || return
    DOCKER_HOST=unix://$CI/docker.sock docker buildx build --load --progress=plain \
      -t "$image:sha-${commit:0:7}" -t "$image:$commit" \
      --label org.opencontainers.image.source=https://github.com/arni-labs/genesis \
      --label org.opencontainers.image.revision="$commit" . || return
    echo "$image:sha-${commit:0:7}" >"$out/image-name.txt"
    DOCKER_HOST=unix://$CI/docker.sock docker image inspect --format '{{.Id}} {{.Size}}' "$image:sha-${commit:0:7}"
}

run_job() {
    local name=$1 s=$SECONDS c
    ( set -o pipefail; eval "job_${1//-/_}" ) >"$out/$name.txt" 2>&1 </dev/null; c=$?
    echo "exit $c" >>"$out/$name.txt"
    printf '%-24s exit %-3s %5s s\n' "$name" "$c" "$((SECONDS - s))" | tee -a "$out/summary.txt" >&3
}
# Docker for the gate's length (a process the run started dies with it); its data and build cache stay in /var/lib/docker.
docker_up() {
    sudo -n dockerd --group "$(id -gn)" -H "unix://$CI/docker.sock" --pidfile "$CI/docker.pid" >"$CI/dockerd.log" 2>&1 &
    for _ in $(seq 60); do DOCKER_HOST=unix://$CI/docker.sock docker info >/dev/null 2>&1 && return; sleep 1; done
    tail -20 "$CI/dockerd.log"; return 1
}
docker_down() { [ -s "$CI/docker.pid" ] && sudo -n kill "$(cat "$CI/docker.pid")" 2>/dev/null; sleep 2; true; }
trap docker_down EXIT
docker_up || { echo "gate: dockerd did not start"; exit 1; }
( peak=0; while sleep 2; do
    used=$(awk '/MemTotal/ {t=$2} /MemAvailable/ {a=$2} END {print int((t - a) / 1024)}' /proc/meminfo)
    [ "$used" -le "$peak" ] || { peak=$used; echo "$peak" >"$out/peak-mem-mb.txt"; }
done ) >/dev/null 2>&1 &
sampler=$!
# workflow-gate needs rust there (needs: rust). The lanes that compile run at nice 10, so the browser tests, which time
# themselves (7.5 s per expect against a vite dev server), keep the cores they need.
( renice -n 10 "$BASHPID" >/dev/null; run_job rust; if grep -q '^rust .*exit 0 ' "$out/summary.txt"; then run_job workflow-gate; fi ) & l1=$!
run_job web & l2=$!
( renice -n 10 "$BASHPID" >/dev/null; run_job image ) & l3=$!
wait $l1 $l2 $l3
kill "$sampler" 2>/dev/null; wait "$sampler" 2>/dev/null
echo "gate: peak memory in use $(cat "$out/peak-mem-mb.txt" 2>/dev/null || echo 0) MB of $(awk '/MemTotal/ {print int($2 / 1024)}' /proc/meminfo) MB"
verdict=passed
grep -q '^rust .*exit 0 ' "$out/summary.txt" || verdict=failed
for f in $(awk '$3 != 0 {print $1}' "$out/summary.txt"); do verdict=failed; echo "== $f (last 40 lines)"; tail -40 "$out/$f.txt"; done
echo "gate: $(wc -l <"$out/summary.txt") jobs, verdict $verdict in $((SECONDS - t0)) s"
said=$(awk '$3 != 0 {printf "%s%s", sep, $1; sep=", "}' "$out/summary.txt")
# A passed push is published when CI_PUBLISH=1: its image to ghcr (the tags image-name.txt lists), with keys/ghcr-token
# (a token that may write packages), as docker.yml pushed it. A push that fails fails the verdict: the Factory's
# railway target deploys only the tag it names.
if [ $verdict = passed ] && [ "${CI_PUBLISH:-0}" = 1 ]; then
    s=$SECONDS
    ( set -e; [ -s "$keys/ghcr-token" ] || { echo "no ghcr-token in $keys"; exit 1; }
      export DOCKER_HOST=unix://$CI/docker.sock DOCKER_CONFIG; DOCKER_CONFIG=$(mktemp -d)
      trap 'rm -rf "$DOCKER_CONFIG"' EXIT
      tr -d '\n' <"$keys/ghcr-token" | docker login ghcr.io -u ci-box --password-stdin
      while read -r tag; do docker push -q "$tag"; done <"$out/image-name.txt" ) >"$out/publish.txt" 2>&1; c=$?
    echo "exit $c" >>"$out/publish.txt"
    printf '%-24s exit %-3s %5s s\n' publish "$c" "$((SECONDS - s))" | tee -a "$out/summary.txt"
    [ $c = 0 ] || { verdict=failed said="publish"; tail -30 "$out/publish.txt"; }
fi

echo "$verdict" >"$out/verdict.txt"
# The verdict to the Factory's gate hook, as ci-temper-v2 and ci-katagami send it: {type, id, tests_url}, signed with
# HMAC-SHA256 of the body under keys/hook-secret.
if [ -n "${GATE_HOOK:-}" ]; then
    if [ -s "$keys/hook-secret" ]; then
        body=$(python3 -c 'import json,sys; print(json.dumps({"type": sys.argv[1], "id": sys.argv[2], "tests_url": sys.argv[3]}))' \
            "$verdict" "$commit" "ci-genesis box gate $verdict${said:+: $said}; $out on 0wtgorbfgesuibjcnvvfn")
        signature=$(printf '%s' "$body" | python3 -c 'import hmac,sys; print(hmac.new(open(sys.argv[1],"rb").read().strip(), sys.stdin.buffer.read(), "sha256").hexdigest())' "$keys/hook-secret")
        curl -sS --fail-with-body --max-time 30 -X POST -H 'content-type: application/json' \
            -H "x-katagami-signature: sha256=$signature" --data "$body" "$GATE_HOOK" && echo "gate: the gate hook took the verdict" ||
            echo "gate: the Factory's gate hook did not take the verdict"
    else echo "gate: no hook-secret in $keys, so the verdict went to no hook"; fi
fi
[ $verdict = passed ]
