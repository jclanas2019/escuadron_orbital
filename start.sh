#!/usr/bin/env bash
# ============================================================================
#  ESCUADRÓN ORBITAL — lanzador autocontenido
#  1) Si hay un binario precompilado compatible (Linux x86_64) lo ejecuta.
#  2) Si no, instala Rust localmente (si falta), las librerías de sistema
#     necesarias (si puede) y compila el juego con Bevy en modo release.
#
#  Uso:  ./start.sh            → jugar
#        ./start.sh --build    → forzar compilación desde el código fuente
# ============================================================================
set -euo pipefail
cd "$(dirname "$(readlink -f "$0" 2>/dev/null || echo "$0")")"

GAME=nave_combate
FORCE_BUILD=0
[[ "${1:-}" == "--build" ]] && FORCE_BUILD=1

say()  { printf '\033[1;32m▶ %s\033[0m\n' "$*"; }
warn() { printf '\033[1;33m⚠ %s\033[0m\n' "$*"; }
die()  { printf '\033[1;31m✖ %s\033[0m\n' "$*"; exit 1; }

OS="$(uname -s)"
ARCH="$(uname -m)"

# ---------------------------------------------------------------- 1. binario precompilado
if [[ $FORCE_BUILD -eq 0 && "$OS" == "Linux" && "$ARCH" == "x86_64" && -x "bin/linux-x86_64/$GAME" ]]; then
    if ! ldd "bin/linux-x86_64/$GAME" 2>/dev/null | grep -q "not found"; then
        say "Iniciando binario precompilado…"
        exec "bin/linux-x86_64/$GAME" "$@"
    else
        warn "Al binario precompilado le faltan librerías:"
        ldd "bin/linux-x86_64/$GAME" | grep "not found" || true
        warn "Intentaré instalarlas y, si no, compilaré desde el código fuente."
    fi
fi

# ---------------------------------------------------------------- 2. librerías del sistema (Linux)
install_linux_deps() {
    local SUDO=""
    [[ $EUID -ne 0 ]] && command -v sudo >/dev/null && SUDO="sudo"
    if command -v apt-get >/dev/null; then
        say "Instalando dependencias (apt)…"
        $SUDO apt-get update -y || true
        $SUDO apt-get install -y build-essential pkg-config curl libasound2-dev libudev-dev \
            libwayland-dev libxkbcommon-dev libx11-dev libvulkan1 mesa-vulkan-drivers || true
    elif command -v dnf >/dev/null; then
        say "Instalando dependencias (dnf)…"
        $SUDO dnf install -y gcc gcc-c++ pkgconf-pkg-config curl alsa-lib-devel systemd-devel \
            wayland-devel libxkbcommon-devel libX11-devel vulkan-loader mesa-vulkan-drivers || true
    elif command -v pacman >/dev/null; then
        say "Instalando dependencias (pacman)…"
        $SUDO pacman -S --needed --noconfirm base-devel pkgconf curl alsa-lib systemd-libs \
            wayland libxkbcommon libx11 vulkan-icd-loader || true
    elif command -v zypper >/dev/null; then
        say "Instalando dependencias (zypper)…"
        $SUDO zypper install -y gcc gcc-c++ pkg-config curl alsa-devel libudev-devel \
            wayland-devel libxkbcommon-devel libX11-devel libvulkan1 || true
    else
        warn "Gestor de paquetes no reconocido: instala manualmente ALSA, udev, Wayland/X11 y Vulkan (dev)."
    fi
}

if [[ "$OS" == "Linux" ]]; then
    need=0
    if command -v pkg-config >/dev/null; then
        pkg-config --exists alsa   || need=1
        pkg-config --exists libudev || need=1
    else
        need=1
    fi
    [[ $need -eq 1 ]] && install_linux_deps
    # Si había binario y solo faltaban librerías de ejecución, reintentar
    if [[ $FORCE_BUILD -eq 0 && "$ARCH" == "x86_64" && -x "bin/linux-x86_64/$GAME" ]] \
        && ! ldd "bin/linux-x86_64/$GAME" 2>/dev/null | grep -q "not found"; then
        say "Iniciando binario precompilado…"
        exec "bin/linux-x86_64/$GAME" "$@"
    fi
elif [[ "$OS" == "Darwin" ]]; then
    xcode-select -p >/dev/null 2>&1 || { warn "Instalando herramientas de línea de comandos de Xcode…"; xcode-select --install || true; }
fi

# ---------------------------------------------------------------- 3. Rust
export CARGO_HOME="${CARGO_HOME:-$HOME/.cargo}"
export RUSTUP_HOME="${RUSTUP_HOME:-$HOME/.rustup}"
export PATH="$CARGO_HOME/bin:$PATH"
if ! command -v cargo >/dev/null; then
    say "Rust no está instalado: instalándolo con rustup (solo para tu usuario)…"
    command -v curl >/dev/null || die "Se necesita 'curl' para instalar Rust."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
    # shellcheck disable=SC1091
    source "$CARGO_HOME/env"
fi
say "Usando $(cargo --version)"

# ---------------------------------------------------------------- 4. compilar y jugar
# En macOS los mandos (Switch Pro, mandos "compatibles con Nintendo", Xbox, DualSense) se leen
# con SDL2, que sí tiene controladores para ellos. Se instala con Homebrew si está disponible.
FEATURES=()
if [[ "$OS" == "Darwin" && -z "${NAVE_SIN_SDL:-}" ]]; then
    if command -v brew >/dev/null; then
        if ! brew list sdl2 >/dev/null 2>&1; then
            say "Instalando SDL2 (para los mandos) con Homebrew…"
            brew install sdl2 || warn "No se pudo instalar SDL2: los mandos usarán el lector básico."
        fi
        if brew list sdl2 >/dev/null 2>&1; then
            BREW_PREFIX="$(brew --prefix)"
            export LIBRARY_PATH="$BREW_PREFIX/lib${LIBRARY_PATH:+:$LIBRARY_PATH}"
            FEATURES=(--features sdl)
            say "Mandos: usando SDL2 de Homebrew."
        fi
    else
        warn "Homebrew no está instalado: los mandos usarán el lector básico. Para el mando Pro de"
        warn "Switch y mandos compatibles instala Homebrew (https://brew.sh) y vuelve a ejecutar start.sh."
    fi
fi
say "Compilando el juego en modo release (la primera vez tarda varios minutos)…"
if ! cargo build --release --locked ${FEATURES[@]+"${FEATURES[@]}"}; then
    if command -v rustup >/dev/null; then
        warn "Falló la compilación; actualizando Rust y reintentando…"
        rustup update stable
        if ! cargo build --release --locked ${FEATURES[@]+"${FEATURES[@]}"}; then
            if [[ ${#FEATURES[@]} -gt 0 ]]; then
                warn "Reintentando sin SDL2…"
                cargo build --release --locked || die "No se pudo compilar. Revisa los mensajes de arriba."
            else
                die "No se pudo compilar. Revisa los mensajes de arriba."
            fi
        fi
    else
        die "No se pudo compilar. Revisa los mensajes de arriba."
    fi
fi
say "¡Despegando!"
exec "target/release/$GAME" "$@"
