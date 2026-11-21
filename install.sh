#!/usr/bin/env bash
# kioku bootstrap: makes sure this checkout has its compiled CLI (bin/kioku) for the version in
# VERSION, then runs `bin/kioku install` with the same arguments (see `bin/kioku install --help`).
#
#   ./install.sh [--vault PATH] [--uninstall] [--no-claude] [--no-codex] …
#
# The binary comes from, in order:
#   1. bin/kioku already there with the right version;
#   2. the GitHub release for this version and platform (sha256 checked);
#   3. a build from source with cargo (KIOKU_BUILD=1 forces this).
set -euo pipefail
export PATH="/etc/profiles/per-user/${USER:-$(id -un)}/bin:/run/current-system/sw/bin:$HOME/.nix-profile/bin:$HOME/.local/bin:$HOME/.cargo/bin:/opt/homebrew/bin:/usr/local/bin:$PATH"

die() { echo "kioku: $*" >&2; exit 1; }
SYS="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
BIN="$SYS/bin/kioku"
VER="$(tr -d '[:space:]' < "$SYS/VERSION")"
REPO="${KIOKU_REPO:-Drum3X/kioku}"

current() { [ -x "$BIN" ] && "$BIN" version 2>/dev/null | grep -q "^$VER "; }

download() {
  local t
  case "$(uname -s)-$(uname -m)" in
    Linux-x86_64) t=x86_64-unknown-linux-gnu ;;
    Linux-aarch64 | Linux-arm64) t=aarch64-unknown-linux-gnu ;;
    Darwin-x86_64) t=x86_64-apple-darwin ;;
    Darwin-arm64) t=aarch64-apple-darwin ;;
    *) return 1 ;;
  esac
  command -v curl >/dev/null || return 1
  local url="https://github.com/$REPO/releases/download/v$VER" tmp
  tmp="$(mktemp -d)"; trap 'rm -rf "$tmp"' RETURN
  curl -fsSL "$url/kioku-$t" -o "$tmp/kioku" && curl -fsSL "$url/SHA256SUMS" -o "$tmp/SHA256SUMS" || return 1
  local want got
  want="$(awk -v f="kioku-$t" '$2 == f {print $1}' "$tmp/SHA256SUMS")"
  if command -v sha256sum >/dev/null; then got="$(sha256sum "$tmp/kioku" | cut -d' ' -f1)"; else got="$(shasum -a 256 "$tmp/kioku" | cut -d' ' -f1)"; fi
  [ -n "$want" ] && [ "$want" = "$got" ] || die "checksum mismatch for kioku-$t from $url"
  rm -f "$BIN"; install -m 755 "$tmp/kioku" "$BIN"
  echo "kioku $VER downloaded ($t)"
}

build() {
  command -v cargo >/dev/null || return 1
  echo "building kioku $VER from source (cargo build --release, with hybrid search) …"
  (cd "$SYS" && { cargo build --release --locked --features semantic-bundled || { echo "kioku: building with hybrid search failed (it needs pkg-config and OpenSSL to fetch ONNX Runtime); building without it" >&2; cargo build --release --locked; }; })
  rm -f "$BIN"; ln -s ../target/release/kioku "$BIN"
}

mkdir -p "$SYS/bin"
if [ -n "${KIOKU_BUILD:-}" ]; then
  build || die "KIOKU_BUILD is set but cargo was not found (https://rustup.rs)"
elif ! current; then
  download || build || die "no kioku binary for $(uname -s)-$(uname -m): no release download for v$VER and no cargo to build it (install Rust from https://rustup.rs, then run this again)"
fi
command -v git >/dev/null || die "'git' is required"
exec "$BIN" install "$@"
