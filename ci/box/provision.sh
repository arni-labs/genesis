#!/usr/bin/env bash
# Installs on ci-genesis (as root) every tool .github/workflows/ci.yml's jobs set up on GitHub's runners, and Docker
# with BuildKit for the image Railway used to build from the Dockerfile. Idempotent: run again (ci/box/box.sh provision)
# after a version below changes. Tools go where they outlive a sleep (the box's own disk); the gate runs as tl-user,
# who owns /var/lib/ci and its caches. Docker's own data (its build cache) stays in /var/lib/docker.
set -euo pipefail
NODE_MAJOR=22                 # actions/setup-node node-version: 22
RUST_NIGHTLY=nightly-2026-02-08  # ci.yml env.RUST_TOOLCHAIN (job rust, with rustfmt)
RUST_STABLE=1.92              # job workflow-gate's toolchain, with wasm32-wasip1 (as the Dockerfile builds them)

CI=/var/lib/ci U=tl-user
export DEBIAN_FRONTEND=noninteractive
step() { echo "provision: $*"; }

step apt
apt-get update -q >/dev/null
apt-get install -yq --no-install-recommends build-essential pkg-config libssl-dev clang libclang-dev libjemalloc-dev \
    libz3-dev git jq curl ca-certificates xz-utils openssl python3 procps psmisc docker.io docker-buildx >/dev/null
systemctl disable --now docker.service docker.socket >/dev/null 2>&1 || true

step "node $NODE_MAJOR"
want=$(curl -fsSL https://nodejs.org/dist/index.json | python3 -c "import json,sys; print(next(r['version'] for r in json.load(sys.stdin) if r['version'].startswith('v$NODE_MAJOR.')))")
if [ "$(/opt/ci/node/bin/node --version 2>/dev/null)" != "$want" ]; then
    rm -rf /opt/ci/node.new && mkdir -p /opt/ci/node.new
    curl -fsSL "https://nodejs.org/dist/$want/node-$want-linux-x64.tar.xz" | tar -xJ -C /opt/ci/node.new --strip-components=1
    rm -rf /opt/ci/node && mv /opt/ci/node.new /opt/ci/node
fi

mkdir -p "$CI" && chown "$U:$U" "$CI" /opt/ci
cat >/opt/ci/env.sh <<ENV
# Sourced by the gate: the tools provision.sh installed, first on PATH, and ci.yml's env.
export RUSTUP_HOME=/opt/ci/rustup CARGO_HOME=/opt/ci/cargo
export PATH=/opt/ci/node/bin:/opt/ci/cargo/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin
export CARGO_TERM_COLOR=never RUST_BACKTRACE=1 RUST_NIGHTLY=$RUST_NIGHTLY RUST_STABLE=$RUST_STABLE
export PLAYWRIGHT_BROWSERS_PATH=/home/$U/.cache/ms-playwright
ENV
chown -R "$U:$U" /opt/ci

step "rust $RUST_NIGHTLY (rustfmt) and $RUST_STABLE (wasm32-wasip1)"
sudo -u "$U" -H bash -c '
    set -euo pipefail; . /opt/ci/env.sh
    command -v rustup >/dev/null || curl -fsSL https://sh.rustup.rs | sh -s -- -y -q --no-modify-path --profile minimal --default-toolchain none
    rustup toolchain install "$RUST_NIGHTLY" --profile minimal -c rustfmt >/dev/null
    rustup toolchain install "$RUST_STABLE" --profile minimal -t wasm32-wasip1 >/dev/null
    rustup default "$RUST_STABLE" >/dev/null
    rustc +"$RUST_NIGHTLY" --version; rustc +"$RUST_STABLE" --version'
step "done: node $(/opt/ci/node/bin/node --version), $(docker --version), buildx $(docker buildx version | cut -d" " -f2)"
