#!/usr/bin/env bash
# Builds Rust Audio Player and installs it as a desktop application
# (binary, icon and .desktop launcher).
#
#   ./install.sh               install for the current user (~/.local)
#   ./install.sh --system      install for all users (/usr/local, uses sudo)
#   sudo ./install.sh          same as --system; still builds as you, not root
#   ./install.sh --uninstall   remove it again (add --system for a system install)
#
# PREFIX=/some/dir ./install.sh overrides the install prefix.
# The library and playlists in ~/.local/share/RustAudioPlayer are never touched.
set -euo pipefail

APP_ID="rust-audio-player"
APP_NAME="Rust Audio Player"

system=false
uninstall=false
for arg in "$@"; do
    case "$arg" in
        --system) system=true ;;
        --uninstall) uninstall=true ;;
        -h | --help)
            sed -n '2,10p' "$0" | sed 's/^# \{0,1\}//'
            exit 0
            ;;
        *)
            echo "Unknown option: $arg (see --help)" >&2
            exit 1
            ;;
    esac
done

# `sudo ./install.sh` means a system-wide install. The build then runs as the
# calling user: root's PATH has no cargo, and a root build would leave
# root-owned files in target/ that break later builds.
build_as=()
build_path="$PATH"
if [[ $EUID -eq 0 && -n "${SUDO_USER:-}" && "$SUDO_USER" != root ]]; then
    system=true
    user_home=$(getent passwd "$SUDO_USER" | cut -d: -f6)
    build_path="$user_home/.cargo/bin:$PATH"
    build_as=(sudo -u "$SUDO_USER" -H)
fi

if $system; then
    PREFIX="${PREFIX:-/usr/local}"
else
    PREFIX="${PREFIX:-$HOME/.local}"
fi

# The prefix ends up inside the .desktop Exec line; keep it to plain paths.
if [[ "$PREFIX" != /* || "$PREFIX" =~ [[:space:]\"\`\$\\%] ]]; then
    echo "PREFIX must be an absolute path without spaces or special characters: $PREFIX" >&2
    exit 1
fi

SUDO=""
if $system && [[ $EUID -ne 0 ]]; then
    SUDO="sudo"
fi

BIN="$PREFIX/bin/$APP_ID"
ICON="$PREFIX/share/icons/hicolor/scalable/apps/$APP_ID.svg"
DESKTOP="$PREFIX/share/applications/$APP_ID.desktop"

# Nearest path at or above $1 that exists.
existing_parent() {
    local dir="$1"
    while [[ ! -e "$dir" ]]; do
        dir=$(dirname "$dir")
    done
    printf '%s\n' "$dir"
}

# Stops before building or writing anything when a target directory is not
# writable, e.g. one left owned by root by another installer run with sudo.
# Without this a failure halfway leaves a partial install behind.
preflight() {
    if [[ -n "$SUDO" || $EUID -eq 0 ]]; then
        return 0
    fi
    local blocked=()
    local file dir
    for file in "$BIN" "$ICON" "$DESKTOP"; do
        # Uninstall only needs to touch files that are actually there.
        if $uninstall && [[ ! -e "$file" ]]; then
            continue
        fi
        dir=$(existing_parent "$(dirname "$file")")
        if [[ ! -d "$dir" || ! -w "$dir" ]]; then
            blocked+=("$dir")
        fi
    done
    if [[ ${#blocked[@]} -eq 0 ]]; then
        return 0
    fi
    echo "Nothing was changed: these directories are not writable by $(id -un):" >&2
    printf '%s\n' "${blocked[@]}" | sort -u | while read -r dir; do
        echo "  $dir (owner: $(stat -c %U "$dir"))" >&2
    done
    echo "Give them back to your user, then run ./install.sh again:" >&2
    printf '%s\n' "${blocked[@]}" | sort -u | while read -r dir; do
        echo "  sudo chown -R $(id -un): $dir" >&2
    done
    echo "Or install for all users instead: sudo ./install.sh" >&2
    exit 1
}

refresh_caches() {
    # All optional: desktops pick the files up on their own, these just make it immediate.
    if command -v update-desktop-database >/dev/null; then
        $SUDO update-desktop-database -q "$PREFIX/share/applications" || true
    fi
    if command -v gtk-update-icon-cache >/dev/null; then
        $SUDO gtk-update-icon-cache -q -f -t "$PREFIX/share/icons/hicolor" || true
    fi
    for tool in kbuildsycoca6 kbuildsycoca5; do
        if command -v "$tool" >/dev/null; then
            # KDE menu cache is per user: rebuild it for the caller, not root.
            "${build_as[@]}" "$tool" --noincremental >/dev/null 2>&1 || true
            break
        fi
    done
}

preflight

if $uninstall; then
    $SUDO rm -f "$BIN" "$ICON" "$DESKTOP"
    refresh_caches
    echo "Removed $APP_NAME from $PREFIX."
    exit 0
fi

cd "$(dirname "$0")"

if ! PATH="$build_path" command -v cargo >/dev/null; then
    echo "cargo not found. Install Rust from https://rustup.rs first." >&2
    exit 1
fi

echo "Building release binary..."
"${build_as[@]}" env "PATH=$build_path" cargo build --release --locked

echo "Installing to $PREFIX..."
$SUDO install -Dm755 target/release/RustAudioPlayer "$BIN"
$SUDO install -Dm644 assets/icon.svg "$ICON"

desktop_entry="[Desktop Entry]
Type=Application
Name=$APP_NAME
GenericName=Audio Player
Comment=Play audio files and manage playlists
Exec=$BIN %F
Icon=$APP_ID
Terminal=false
Categories=AudioVideo;Audio;Player;
MimeType=audio/mpeg;audio/flac;audio/x-flac;audio/ogg;audio/x-vorbis+ogg;audio/wav;audio/x-wav;audio/mp4;audio/aac;
Keywords=music;audio;player;playlist;mp3;flac;
StartupWMClass=$APP_ID
"
printf '%s' "$desktop_entry" | $SUDO install -Dm644 /dev/stdin "$DESKTOP"

refresh_caches

echo "Installed $APP_NAME:"
echo "  binary   $BIN"
echo "  icon     $ICON"
echo "  launcher $DESKTOP"
if [[ ":$PATH:" != *":$PREFIX/bin:"* ]]; then
    echo "Note: $PREFIX/bin is not in PATH; the menu launcher works anyway."
fi
