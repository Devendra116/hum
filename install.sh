#!/bin/sh
# install.sh — one-command installer for hum
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/Devendra116/hum/main/install.sh | sh
#
# Pin a version:
#   curl -fsSL https://raw.githubusercontent.com/Devendra116/hum/main/install.sh | HUM_VERSION=v0.1.0 sh
#
# Custom install dir:
#   curl -fsSL ... | HUM_INSTALL_DIR=/usr/local/bin sh
#
# Security:
#   - All downloads over HTTPS from official GitHub repos only
#   - SHA-256 checksum verification for hum and yt-dlp binaries
#   - Installs to user directory (~/.local/bin) by default — no sudo for binaries
#   - Temp files cleaned up on exit (trap)
#   - Fails closed on unknown OS/arch

set -eu

REPO="Devendra116/hum"
BINARY="hum"
INSTALL_DIR="${HUM_INSTALL_DIR:-$HOME/.local/bin}"

YTDLP_REPO="yt-dlp/yt-dlp"

# ── helpers ──────────────────────────────────────────────────────────────────

info()  { printf '\033[1;34m[info]\033[0m  %s\n' "$1"; }
ok()    { printf '\033[1;32m[ ok ]\033[0m  %s\n' "$1"; }
warn()  { printf '\033[1;33m[warn]\033[0m  %s\n' "$1"; }
error() { printf '\033[1;31m[error]\033[0m %s\n' "$1" >&2; exit 1; }

need() {
    command -v "$1" >/dev/null 2>&1 || error "Required tool '$1' not found. Please install it first."
}

sha256_verify() {
    file="$1"
    expected="$2"
    label="$3"
    if command -v sha256sum >/dev/null 2>&1; then
        actual=$(sha256sum "$file" | cut -d' ' -f1)
    elif command -v shasum >/dev/null 2>&1; then
        actual=$(shasum -a 256 "$file" | cut -d' ' -f1)
    else
        warn "Neither sha256sum nor shasum found — skipping checksum verification for $label"
        return 0
    fi
    if [ "$actual" != "$expected" ]; then
        error "Checksum mismatch for $label!\n  expected: $expected\n  got:      $actual\nThe download may be corrupted or tampered with. Aborting."
    fi
    ok "Checksum verified for $label"
}

# ── detect platform ──────────────────────────────────────────────────────────

detect_platform() {
    UNAME_OS=$(uname -s)
    ARCH=$(uname -m)

    case "$UNAME_OS" in
        Linux*)  OS_TYPE="linux";  RUST_OS="unknown-linux-gnu" ;;
        Darwin*) OS_TYPE="macos";  RUST_OS="apple-darwin" ;;
        *)       error "Unsupported OS: $UNAME_OS. This installer supports Linux and macOS.\nFor Windows, use install.ps1 instead." ;;
    esac

    case "$ARCH" in
        x86_64|amd64)   ARCH="x86_64" ;;
        aarch64|arm64)   ARCH="aarch64" ;;
        *)               error "Unsupported architecture: $ARCH" ;;
    esac

    TARGET="${ARCH}-${RUST_OS}"
    info "Detected platform: $TARGET ($OS_TYPE)"
}

# ── resolve version ──────────────────────────────────────────────────────────

resolve_version() {
    need curl

    if [ -n "${HUM_VERSION:-}" ]; then
        VERSION="$HUM_VERSION"
        info "Using pinned version: $VERSION"
        return
    fi

    info "Fetching latest release…"
    VERSION=$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" \
        | grep '"tag_name"' \
        | head -1 \
        | sed 's/.*"tag_name": *"//;s/".*//')

    if [ -z "$VERSION" ]; then
        error "Could not determine latest version. Set HUM_VERSION manually."
    fi
    info "Latest version: $VERSION"
}

# ── check for existing install ───────────────────────────────────────────────

check_existing() {
    if command -v "$BINARY" >/dev/null 2>&1; then
        CURRENT=$("$BINARY" --version 2>/dev/null | head -1 || echo "unknown")
        info "Found existing install: $CURRENT"

        CLEAN_VERSION=$(echo "$VERSION" | sed 's/^v//')
        if echo "$CURRENT" | grep -q "$CLEAN_VERSION"; then
            info "Already up to date ($VERSION). Nothing to do."
            exit 0
        fi
        info "Upgrading to $VERSION…"
    else
        info "No existing install found. Installing $VERSION…"
    fi
}

# ── download & install hum ───────────────────────────────────────────────────

download_and_install() {
    need curl
    need tar

    ASSET="${BINARY}-${VERSION}-${TARGET}.tar.gz"
    CHECKSUMS="${BINARY}-${VERSION}-checksums.txt"
    BASE_URL="https://github.com/${REPO}/releases/download/${VERSION}"

    TMPDIR=$(mktemp -d)
    trap 'rm -rf "$TMPDIR"' EXIT

    info "Downloading $ASSET…"
    curl -fSL "${BASE_URL}/${ASSET}" -o "${TMPDIR}/${ASSET}"

    info "Downloading checksums…"
    if curl -fSL "${BASE_URL}/${CHECKSUMS}" -o "${TMPDIR}/${CHECKSUMS}" 2>/dev/null; then
        EXPECTED=$(grep "$ASSET" "${TMPDIR}/${CHECKSUMS}" | cut -d' ' -f1)
        if [ -n "$EXPECTED" ]; then
            sha256_verify "${TMPDIR}/${ASSET}" "$EXPECTED" "hum"
        else
            warn "Asset not found in checksums file — skipping verification"
        fi
    else
        warn "Checksums file not available — skipping verification"
    fi

    info "Extracting…"
    tar -xzf "${TMPDIR}/${ASSET}" -C "${TMPDIR}"

    mkdir -p "$INSTALL_DIR"

    if [ -f "${TMPDIR}/${BINARY}" ]; then
        mv "${TMPDIR}/${BINARY}" "${INSTALL_DIR}/${BINARY}"
    elif [ -f "${TMPDIR}/${BINARY}-${VERSION}-${TARGET}/${BINARY}" ]; then
        mv "${TMPDIR}/${BINARY}-${VERSION}-${TARGET}/${BINARY}" "${INSTALL_DIR}/${BINARY}"
    else
        error "Could not find '${BINARY}' binary in the archive"
    fi

    chmod +x "${INSTALL_DIR}/${BINARY}"
    ok "Installed hum to ${INSTALL_DIR}/${BINARY}"
}

# ── install yt-dlp (standalone binary, no Python) ────────────────────────────

install_ytdlp() {
    if command -v yt-dlp >/dev/null 2>&1; then
        ok "yt-dlp already installed"
        return
    fi

    info "Installing yt-dlp (standalone binary — no Python required)…"

    case "$OS_TYPE" in
        linux)  YTDLP_ASSET="yt-dlp_linux"      ;;
        macos)  YTDLP_ASSET="yt-dlp_macos"       ;;
        *)      warn "Cannot auto-install yt-dlp on $OS_TYPE"; return 1 ;;
    esac

    case "$ARCH" in
        aarch64)
            if [ "$OS_TYPE" = "linux" ]; then
                YTDLP_ASSET="yt-dlp_linux_aarch64"
            fi
            ;;
    esac

    YTDLP_URL="https://github.com/${YTDLP_REPO}/releases/latest/download/${YTDLP_ASSET}"
    YTDLP_SUMS_URL="https://github.com/${YTDLP_REPO}/releases/latest/download/SHA2-256SUMS"

    YTDLP_TMP=$(mktemp -d)

    info "Downloading ${YTDLP_ASSET}…"
    if ! curl -fSL "$YTDLP_URL" -o "${YTDLP_TMP}/${YTDLP_ASSET}"; then
        warn "Failed to download yt-dlp. You'll need to install it manually."
        rm -rf "$YTDLP_TMP"
        return 1
    fi

    info "Verifying yt-dlp checksum…"
    if curl -fSL "$YTDLP_SUMS_URL" -o "${YTDLP_TMP}/SHA2-256SUMS" 2>/dev/null; then
        YTDLP_EXPECTED=$(grep "${YTDLP_ASSET}$" "${YTDLP_TMP}/SHA2-256SUMS" | cut -d' ' -f1)
        if [ -n "$YTDLP_EXPECTED" ]; then
            sha256_verify "${YTDLP_TMP}/${YTDLP_ASSET}" "$YTDLP_EXPECTED" "yt-dlp"
        else
            warn "yt-dlp asset not found in checksums — skipping verification"
        fi
    else
        warn "yt-dlp checksums not available — skipping verification"
    fi

    mkdir -p "$INSTALL_DIR"
    mv "${YTDLP_TMP}/${YTDLP_ASSET}" "${INSTALL_DIR}/yt-dlp"
    chmod +x "${INSTALL_DIR}/yt-dlp"
    rm -rf "$YTDLP_TMP"
    ok "Installed yt-dlp to ${INSTALL_DIR}/yt-dlp"
}

# ── install mpv ──────────────────────────────────────────────────────────────

install_mpv() {
    if command -v mpv >/dev/null 2>&1; then
        ok "mpv already installed"
        return
    fi

    info "mpv is required for audio playback. Attempting to install…"

    if command -v brew >/dev/null 2>&1; then
        info "Installing mpv via Homebrew…"
        if brew install mpv; then
            ok "mpv installed via Homebrew"
            return
        fi
    fi

    if [ "$OS_TYPE" = "linux" ]; then
        if command -v apt-get >/dev/null 2>&1; then
            info "Installing mpv via apt (requires sudo)…"
            if sudo apt-get install -y mpv; then
                ok "mpv installed via apt"
                return
            fi
        elif command -v pacman >/dev/null 2>&1; then
            info "Installing mpv via pacman (requires sudo)…"
            if sudo pacman -S --noconfirm mpv; then
                ok "mpv installed via pacman"
                return
            fi
        elif command -v dnf >/dev/null 2>&1; then
            info "Installing mpv via dnf (requires sudo)…"
            if sudo dnf install -y mpv; then
                ok "mpv installed via dnf"
                return
            fi
        elif command -v zypper >/dev/null 2>&1; then
            info "Installing mpv via zypper (requires sudo)…"
            if sudo zypper install -y mpv; then
                ok "mpv installed via zypper"
                return
            fi
        elif command -v apk >/dev/null 2>&1; then
            info "Installing mpv via apk (requires sudo)…"
            if sudo apk add mpv; then
                ok "mpv installed via apk"
                return
            fi
        fi
    fi

    warn "Could not auto-install mpv"
    echo ""
    echo "  Please install mpv manually:"
    echo ""
    if [ "$OS_TYPE" = "macos" ]; then
        echo "    brew install mpv"
    else
        echo "    sudo apt install mpv        # Debian/Ubuntu"
        echo "    sudo pacman -S mpv          # Arch"
        echo "    sudo dnf install mpv        # Fedora"
    fi
    echo ""
    echo "  More info: https://mpv.io/installation/"
    echo ""
}

# ── PATH check ───────────────────────────────────────────────────────────────

check_path() {
    case ":${PATH}:" in
        *":${INSTALL_DIR}:"*) ;;
        *)
            warn "${INSTALL_DIR} is not in your PATH"
            echo ""
            echo "  Add it to your shell config:"
            echo "    echo 'export PATH=\"${INSTALL_DIR}:\$PATH\"' >> ~/.bashrc"
            echo "    echo 'export PATH=\"${INSTALL_DIR}:\$PATH\"' >> ~/.zshrc   # if using zsh"
            echo ""
            echo "  Then reload:  source ~/.bashrc  (or restart your terminal)"
            echo ""
            ;;
    esac
}

# ── final status ─────────────────────────────────────────────────────────────

print_status() {
    echo ""
    echo "  ────────────────────────────────────"
    echo "  Status:"
    echo ""

    HUM_OK=false; YTDLP_OK=false; MPV_OK=false

    if command -v hum >/dev/null 2>&1 || [ -x "${INSTALL_DIR}/hum" ]; then
        ok "hum     installed"; HUM_OK=true
    else
        warn "hum     not in PATH (installed to ${INSTALL_DIR}/hum)"
    fi

    if command -v yt-dlp >/dev/null 2>&1 || [ -x "${INSTALL_DIR}/yt-dlp" ]; then
        ok "yt-dlp  installed"; YTDLP_OK=true
    else
        warn "yt-dlp  missing"
    fi

    if command -v mpv >/dev/null 2>&1; then
        ok "mpv     installed"; MPV_OK=true
    else
        warn "mpv     missing — audio won't work until installed"
    fi

    echo "  ────────────────────────────────────"
    echo ""

    if [ "$HUM_OK" = true ] && [ "$YTDLP_OK" = true ] && [ "$MPV_OK" = true ]; then
        ok "All good! Run 'hum' to start playing music."
    else
        info "hum is installed but some dependencies need attention (see above)."
    fi
    echo ""
}

# ── main ─────────────────────────────────────────────────────────────────────

main() {
    echo ""
    echo "  ♪ hum installer"
    echo ""

    detect_platform
    resolve_version
    check_existing
    download_and_install
    install_ytdlp
    install_mpv
    check_path
    print_status
}

main
