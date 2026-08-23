#!/usr/bin/env bash
# Install a prebuilt talaria binary from GitHub Releases.
#
#   curl -fsSL https://raw.githubusercontent.com/inaki/talaria/main/install.sh | bash
#   curl -fsSL ... | bash -s -- v0.1.4
#
# Does not install Hermes Agent. Live mode still needs ~/.hermes (shared).
# Talaria is an unofficial host — see NOTICE.md.

set -euo pipefail

REPO="${TALARIA_REPO:-${HERMES_RUST_REPO:-inaki/talaria}}"
BIN_NAME="talaria"
BIN_DIR="${TALARIA_BIN_DIR:-${HERMES_RUST_BIN_DIR:-$HOME/.local/bin}}"
VERSION="${1:-${TALARIA_VERSION:-${HERMES_RUST_VERSION:-latest}}}"

red() { printf '\033[31m%s\033[0m\n' "$*"; }
green() { printf '\033[32m%s\033[0m\n' "$*"; }
cyan() { printf '\033[36m%s\033[0m\n' "$*"; }

die() {
  red "error: $*"
  exit 1
}

need() {
  command -v "$1" >/dev/null 2>&1 || die "need $1 on PATH"
}

detect_target() {
  local os arch
  os="$(uname -s)"
  arch="$(uname -m)"
  case "$os" in
    Darwin)
      case "$arch" in
        arm64) echo "aarch64-apple-darwin" ;;
        x86_64) echo "x86_64-apple-darwin" ;;
        *) die "unsupported macOS arch: $arch" ;;
      esac
      ;;
    Linux)
      case "$arch" in
        x86_64) echo "x86_64-unknown-linux-gnu" ;;
        aarch64 | arm64) echo "aarch64-unknown-linux-gnu" ;;
        *) die "unsupported Linux arch: $arch" ;;
      esac
      ;;
    *)
      die "unsupported OS: $os (Windows is not in v1)"
      ;;
  esac
}

file_sha256() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | awk '{print $1}'
  elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$1" | awk '{print $1}'
  else
    die "need sha256sum or shasum"
  fi
}

resolve_tag() {
  local ver="$1"
  if [ "$ver" = "latest" ]; then
    need curl
    curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" \
      | sed -n 's/.*"tag_name": *"\([^"]*\)".*/\1/p' \
      | head -1
  else
    case "$ver" in
      v*) echo "$ver" ;;
      *) echo "v${ver}" ;;
    esac
  fi
}

main() {
  need curl
  need tar
  need mktemp
  need uname

  local target tag version asset url sums tmp dir expected got
  target="$(detect_target)"
  tag="$(resolve_tag "$VERSION")"
  [ -n "$tag" ] || die "could not resolve release tag (set TALARIA_VERSION or pass v0.1.4)"
  version="${tag#v}"
  asset="${BIN_NAME}-${version}-${target}.tar.gz"
  url="https://github.com/${REPO}/releases/download/${tag}/${asset}"
  sums="https://github.com/${REPO}/releases/download/${tag}/SHA256SUMS"

  cyan "→ installing ${BIN_NAME} ${tag} (${target})"
  tmp="$(mktemp -d)"
  trap 'rm -rf "$tmp"' EXIT

  curl -fsSL "$url" -o "${tmp}/${asset}" || die "download failed: $url"
  if curl -fsSL "$sums" -o "${tmp}/SHA256SUMS"; then
    expected="$(awk -v f="$asset" '$2 == f { print $1 }' "${tmp}/SHA256SUMS")"
    [ -n "$expected" ] || die "SHA256SUMS has no entry for ${asset}"
    got="$(file_sha256 "${tmp}/${asset}")"
    [ "$expected" = "$got" ] || die "checksum mismatch for ${asset}"
    green "✓ checksum ok"
  else
    die "could not download SHA256SUMS (refusing unsigned install)"
  fi

  tar -xzf "${tmp}/${asset}" -C "$tmp"
  [ -f "${tmp}/${BIN_NAME}" ] || die "archive missing ${BIN_NAME}"
  mkdir -p "$BIN_DIR"
  install -m 755 "${tmp}/${BIN_NAME}" "${BIN_DIR}/${BIN_NAME}"
  green "✓ ${BIN_DIR}/${BIN_NAME}"

  case ":$PATH:" in
    *":${BIN_DIR}:"*) ;;
    *)
      cyan "→ add to PATH:"
      echo "  export PATH=\"${BIN_DIR}:\$PATH\""
      echo "  # then reload the shell, or append that line to ~/.zshrc / ~/.bashrc"
      ;;
  esac

  echo
  "${BIN_DIR}/${BIN_NAME}" --version || true
  echo
  echo "Talaria is an unofficial TUI client for Hermes Agent (Nous Research)."
  echo "Live mode uses ~/.hermes (shared). Quit hermes --tui before starting talaria."
  echo "If live mode cannot import tui_gateway, install Hermes Agent first:"
  echo "  https://hermes-agent.nousresearch.com"
}

main "$@"
