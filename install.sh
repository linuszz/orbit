#!/bin/sh
set -e

REPO="linuszz/orbt"
BIN="orbt"
INSTALL_DIR="${ORBT_INSTALL_DIR:-/usr/local/bin}"

# Detect OS and arch
OS="$(uname -s)"
ARCH="$(uname -m)"

case "$OS" in
  Linux)
    case "$ARCH" in
      x86_64) TARGET="linux-x86_64" ;;
      *)
        echo "Unsupported architecture: $ARCH" >&2
        exit 1
        ;;
    esac
    ;;
  Darwin)
    case "$ARCH" in
      arm64)  TARGET="macos-aarch64" ;;
      x86_64) TARGET="macos-x86_64" ;;
      *)
        echo "Unsupported architecture: $ARCH" >&2
        exit 1
        ;;
    esac
    ;;
  *)
    echo "Unsupported OS: $OS" >&2
    exit 1
    ;;
esac

# Resolve latest version
if [ -z "$ORBT_VERSION" ]; then
  VERSION="$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" \
    | grep '"tag_name"' | sed 's/.*"tag_name": *"\([^"]*\)".*/\1/')"
else
  VERSION="$ORBT_VERSION"
fi

if [ -z "$VERSION" ]; then
  echo "Failed to resolve latest version" >&2
  exit 1
fi

URL="https://github.com/${REPO}/releases/download/${VERSION}/orbt-${TARGET}.tar.gz"

echo "Installing orbt ${VERSION} (${TARGET})..."

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

curl -fsSL "$URL" | tar xz -C "$TMP"

# Install — try sudo if INSTALL_DIR is not writable
if [ -w "$INSTALL_DIR" ]; then
  cp "$TMP/$BIN" "$INSTALL_DIR/$BIN"
  chmod 755 "$INSTALL_DIR/$BIN"
else
  echo "Installing to $INSTALL_DIR (requires sudo)..."
  sudo cp "$TMP/$BIN" "$INSTALL_DIR/$BIN"
  sudo chmod 755 "$INSTALL_DIR/$BIN"
fi

echo "Installed: $("$INSTALL_DIR/$BIN" --version 2>/dev/null || echo "$INSTALL_DIR/$BIN")"
echo "Run 'orbt' to start."
