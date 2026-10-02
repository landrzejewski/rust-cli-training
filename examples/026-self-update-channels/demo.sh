#!/usr/bin/env bash
# End-to-end self-update demo against a local manifest feed (python3 http.server):
# builds 4 "releases" from the same sources, installs 0.1.0 and lets it update itself.
set -euo pipefail
cd "$(dirname "$0")/../.."
PKG=ex026-self-update-channels
BIN=platctl-up
TARGET=$(rustc -vV | sed -n 's/^host: //p')
BUILDS=dist/platctl-up-builds   # pristine archives
FEED=dist/platctl-up-feed       # what the "server" publishes
INSTALL=dist/platctl-up-install # the user's machine
PORT=${PORT:-18765}
rm -rf "$BUILDS" "$FEED" "$INSTALL" && mkdir -p "$BUILDS" "$FEED" "$INSTALL/bin" "$INSTALL/state"

# 1. Build releases – only the compiled-in version differs.
for version in 0.1.0 0.2.0 0.3.0-beta.1 0.3.0; do
  PLATCTL_UP_BUILD_VERSION=$version cargo build -q --release -p $PKG
  stage=$(mktemp -d) && cp target/release/$BIN "$stage/"
  tar -C "$stage" -czf "$BUILDS/$BIN-$version-$TARGET.tar.gz" $BIN && rm -rf "$stage"
done

# 2. publish <versions...>: copy archives to the feed, write manifest.json with sha256 digests.
publish() {
  local entries="" file sha
  for version in "$@"; do
    file=$BIN-$version-$TARGET.tar.gz
    cp "$BUILDS/$file" "$FEED/"
    sha=$(shasum -a 256 "$FEED/$file" | cut -d' ' -f1)
    entries+="${entries:+,}{\"version\":\"$version\",\"assets\":[{\"name\":\"$file\",\"url\":\"$file\",\"digest\":\"sha256:$sha\"}]}"
  done
  echo "{\"schema\":1,\"releases\":[$entries]}" > "$FEED/manifest.json"
}
publish 0.1.0 0.2.0 0.3.0-beta.1
if lsof -iTCP:$PORT -sTCP:LISTEN >/dev/null 2>&1; then echo "port $PORT is busy – run with PORT=<free port>"; exit 1; fi
python3 -m http.server $PORT --bind 127.0.0.1 -d "$FEED" >/dev/null 2>&1 &
SERVER=$!
trap 'kill $SERVER' EXIT
sleep 1

# 3. "Install" 0.1.0 and point it at the feed; state (channel, last check) in a private dir.
tar -xzf "$BUILDS/$BIN-0.1.0-$TARGET.tar.gz" -C "$INSTALL/bin"
export PLATCTL_UP_FEED=http://127.0.0.1:$PORT PLATCTL_UP_STATE_DIR=$INSTALL/state
up=$INSTALL/bin/$BIN

step() { printf '\n$ %s\n' "${*#"$INSTALL/bin/"}"; "$@" || echo "[exit code $?]"; }
# The update notice only appears on a terminal – `script` provides one.
on_tty() {
  if [[ $(uname) == Darwin ]]; then script -q /dev/null "$@"; else script -qec "$*" /dev/null; fi </dev/null | sed 's/\^D//g'
}

step "$up" --version
step on_tty "$up" status                 # first run of the day: notice on stderr
step on_tty "$up" status                 # throttled: no second check within 24 h
step "$up" status                        # not a terminal: never a notice
step "$up" update --check                # exit code 10 = update available
step "$up" releases --channel beta
step "$up" update                        # no terminal, no --yes: refuses to prompt
step "$up" update --yes
step "$up" --version
step "$up" channel beta
step "$up" update --yes
step "$up" --version
step "$up" channel stable
step "$up" update --check                # a beta stays until a newer stable exists

# 4. 0.3.0 ships, but the archive on the server is corrupted after the manifest was written.
publish 0.1.0 0.2.0 0.3.0-beta.1 0.3.0
echo "garbage" >> "$FEED/$BIN-0.3.0-$TARGET.tar.gz"
step "$up" update --yes                  # digest mismatch -> nothing replaced
step "$up" --version
publish 0.1.0 0.2.0 0.3.0-beta.1 0.3.0   # fixed upload
step "$up" update --yes                  # 0.3.0-beta.1 -> 0.3.0 (final > its betas)
step "$up" --version
