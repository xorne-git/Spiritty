#!/usr/bin/env bash
# ==============================================================================
#  🧞 Spiritty — Official One-Line Installer
#  Usage:
#    curl -fsSL https://raw.githubusercontent.com/xorne-git/Spiritty/main/install.sh | bash
# ==============================================================================

set -e

# --- Visual Styling ---
BOLD='\033[1m'
CYAN='\033[0;36m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
RED='\033[0;31m'
GRAY='\033[0;90m'
NC='\033[0m' # No Color

REPO="xorne-git/Spiritty"
BINARY_NAME="spiritty"

print_banner() {
    printf "${CYAN}${BOLD}"
    cat << 'EOF'
   _____       _      _ _   _         🧞
  / ____|     (_)    (_) | | |        
 | (___  _ __  _ _ __ _| |_| |_ _   _ 
  \___ \| '_ \| | '__| | __| __| | | |
  ____) | |_) | | |  | | |_| |_| |_| |
 |_____/| .__/|_|_|  |_|\__|\__|\__, |
        | |                      __/ |
        |_|                     |___/ 
EOF
    printf "${NC}\n"
    echo -e "${BOLD}L'assistant IA pour terminal nouvelle génération${NC}"
    echo -e "${GRAY}https://github.com/${REPO}${NC}"
    echo ""
}

info() {
    echo -e "${CYAN}==>${NC} ${BOLD}$1${NC}"
}

success() {
    echo -e "${GREEN}✓${NC} ${BOLD}$1${NC}"
}

warn() {
    echo -e "${YELLOW}⚠️  $1${NC}"
}

error() {
    echo -e "${RED}❌ Erreur : $1${NC}" >&2
    exit 1
}

# --- 1. Detect OS & Architecture ---
detect_platform() {
    OS="$(uname -s)"
    ARCH="$(uname -m)"

    case "$OS" in
        Linux)
            TARGET_OS="unknown-linux-gnu"
            ;;
        Darwin)
            TARGET_OS="apple-darwin"
            ;;
        *)
            error "Système d'exploitation non supporté : $OS. Spiritty fonctionne sur Linux et macOS."
            ;;
    esac

    case "$ARCH" in
        x86_64|amd64)
            TARGET_ARCH="x86_64"
            ;;
        aarch64|arm64)
            TARGET_ARCH="aarch64"
            ;;
        *)
            error "Architecture non supportée : $ARCH. Spiritty supporte x86_64 et aarch64 (ARM64)."
            ;;
    esac

    TARGET="${TARGET_ARCH}-${TARGET_OS}"
}

# --- 2. Check for required tools ---
check_dependencies() {
    if command -v curl >/dev/null 2>&1; then
        FETCH_CMD="curl -fsSL"
    elif command -v wget >/dev/null 2>&1; then
        FETCH_CMD="wget -qO-"
    else
        error "Ni 'curl' ni 'wget' n'ont été trouvés. Veuillez installer curl ou wget."
    fi

    if ! command -v tar >/dev/null 2>&1; then
        error "'tar' est requis pour extraire l'archive d'installation."
    fi
}

# --- 3. Determine latest version ---
get_latest_version() {
    info "Recherche de la dernière version disponible..."
    
    # Try fetching the latest release from GitHub API
    LATEST_TAG=$(curl -s "https://api.github.com/repos/${REPO}/releases/latest" | grep '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/' || true)

    if [ -z "$LATEST_TAG" ] || [ "$LATEST_TAG" = "null" ]; then
        # Fallback to hardcoded current release if GitHub API is rate limited
        LATEST_TAG="v0.6.4"
        warn "Impossible de contacter l'API GitHub (limite de requêtes atteinte), utilisation de la version ${LATEST_TAG}."
    fi

    success "Version sélectionnée : ${LATEST_TAG} (${TARGET})"
}

# --- 4. Choose installation directory ---
get_install_dir() {
    if [ -w "/usr/local/bin" ]; then
        INSTALL_DIR="/usr/local/bin"
    elif [ -d "$HOME/.local/bin" ] || mkdir -p "$HOME/.local/bin" 2>/dev/null; then
        INSTALL_DIR="$HOME/.local/bin"
    else
        INSTALL_DIR="/usr/local/bin"
    fi
}

# --- 5. Download and install binary ---
install_binary() {
    TMP_DIR=$(mktemp -d)
    trap 'rm -rf "$TMP_DIR"' EXIT

    ARCHIVE_NAME="spiritty-${TARGET}.tar.gz"
    DOWNLOAD_URL="https://github.com/${REPO}/releases/download/${LATEST_TAG}/${ARCHIVE_NAME}"

    info "Téléchargement de Spiritty depuis GitHub..."
    echo -e "${GRAY}${DOWNLOAD_URL}${NC}"

    HTTP_STATUS=$(curl -s -w "%{http_code}" -L "$DOWNLOAD_URL" -o "${TMP_DIR}/${ARCHIVE_NAME}")

    if [ "$HTTP_STATUS" -ne 200 ]; then
        error "Échec du téléchargement (HTTP $HTTP_STATUS). La release pour '${TARGET}' n'est peut-être pas encore disponible."
    fi

    info "Extraction de l'archive..."
    tar -xzf "${TMP_DIR}/${ARCHIVE_NAME}" -C "${TMP_DIR}"

    # Integrity check against the published .sha256 (skip with a warning when absent,
    # hard-fail on mismatch — never install a corrupted or tampered binary).
    info "Vérification de l'intégrité (sha256)..."
    if curl -fsSL -o "${TMP_DIR}/${ARCHIVE_NAME}.sha256" "${DOWNLOAD_URL}.sha256"; then
        # Precedence matters: without the { } group, `| awk` bound only to the
        # `shasum` fallback — on Linux sha256sum succeeded and its RAW stdin
        # output ("<hash>  -") was captured, so the comparison always failed.
        COMPUTED="$({ command sha256sum < "${TMP_DIR}/${ARCHIVE_NAME}" 2>/dev/null \
            || command shasum -a 256 < "${TMP_DIR}/${ARCHIVE_NAME}"; } | awk '{print $1}')"
        PUBLISHED="$(awk 'NR==1{print tolower($1)}' "${TMP_DIR}/${ARCHIVE_NAME}.sha256")"
        if [ -z "$COMPUTED" ] || [ "$COMPUTED" != "$PUBLISHED" ]; then
            error "Somme de contrôle invalide (attendu ${PUBLISHED:-?}, obtenu ${COMPUTED:-aucun}). Installation annulée."
        fi
        success "Somme de contrôle valide."
    else
        warn "Fichier .sha256 indisponible pour cette release, vérification ignorée."
    fi

    if [ ! -f "${TMP_DIR}/${BINARY_NAME}" ]; then
        error "Le binaire 'spiritty' n'a pas été trouvé dans l'archive téléchargée."
    fi

    chmod +x "${TMP_DIR}/${BINARY_NAME}"

    info "Installation dans ${INSTALL_DIR}/${BINARY_NAME}..."
    if [ -w "$INSTALL_DIR" ]; then
        mv "${TMP_DIR}/${BINARY_NAME}" "${INSTALL_DIR}/${BINARY_NAME}"
    else
        echo -e "${YELLOW}Droits administrateur requis pour installer dans ${INSTALL_DIR}.${NC}"
        sudo mv "${TMP_DIR}/${BINARY_NAME}" "${INSTALL_DIR}/${BINARY_NAME}"
    fi

    success "Spiritty installé avec succès dans ${INSTALL_DIR}/${BINARY_NAME} !"
}

# --- 6. Verify PATH ---
check_path() {
    case ":$PATH:" in
        *":$INSTALL_DIR:"*) ;;
        *)
            echo ""
            warn "${INSTALL_DIR} ne semble pas être dans votre variable \$PATH."
            echo "Ajoutez cette ligne à votre fichier de configuration de shell :"
            echo ""
            if [ -n "${ZSH_VERSION:-}" ] || [ -f "$HOME/.zshrc" ]; then
                echo -e "  ${BOLD}echo 'export PATH=\"\$HOME/.local/bin:\$PATH\"' >> ~/.zshrc && source ~/.zshrc${NC}"
            elif [ -n "${BASH_VERSION:-}" ] || [ -f "$HOME/.bashrc" ]; then
                echo -e "  ${BOLD}echo 'export PATH=\"\$HOME/.local/bin:\$PATH\"' >> ~/.bashrc && source ~/.bashrc${NC}"
            elif [ -f "$HOME/.config/fish/config.fish" ]; then
                echo -e "  ${BOLD}fish_add_path ~/.local/bin${NC}"
            else
                echo -e "  ${BOLD}export PATH=\"\$HOME/.local/bin:\$PATH\"${NC}"
            fi
            echo ""
            ;;
    esac
}

# --- 7. Desktop Environment & Launcher Entry ---
detect_desktop_environment() {
    DESKTOP_ENV=""
    
    local xdg_desktop="${XDG_CURRENT_DESKTOP:-${DESKTOP_SESSION:-}}"
    
    if [[ "$xdg_desktop" =~ (GNOME|gnome) ]] || pgrep -x gnome-shell >/dev/null 2>&1; then
        DESKTOP_ENV="GNOME"
    elif [[ "$xdg_desktop" =~ (KDE|Kde|plasma) ]] || pgrep -f kwin >/dev/null 2>&1; then
        DESKTOP_ENV="KDE Plasma"
    elif [[ "$xdg_desktop" =~ (XFCE|xfce) ]] || pgrep -x xfce4-session >/dev/null 2>&1; then
        DESKTOP_ENV="XFCE"
    elif [[ "$xdg_desktop" =~ (Hyprland|hyprland) ]] || pgrep -x Hyprland >/dev/null 2>&1; then
        DESKTOP_ENV="Hyprland"
    elif [[ "$xdg_desktop" =~ (sway|Sway) ]] || pgrep -x sway >/dev/null 2>&1; then
        DESKTOP_ENV="Sway"
    elif systemctl --user is-active dms.service >/dev/null 2>&1 || [ -d "$HOME/.config/dms" ] || [ -d "$HOME/.config/DankMaterialShell" ]; then
        DESKTOP_ENV="DankMaterialShell (DMS)"
    elif [ -n "$xdg_desktop" ]; then
        DESKTOP_ENV="$xdg_desktop"
    elif [ -n "${WAYLAND_DISPLAY:-}" ] || [ -n "${DISPLAY:-}" ]; then
        DESKTOP_ENV="Environnement graphique"
    fi
}

setup_desktop_entry() {
    if [ "$OS" != "Linux" ]; then
        return 0
    fi

    detect_desktop_environment

    if [ -z "$DESKTOP_ENV" ]; then
        # Headless server without GUI/display, skip quietly
        return 0
    fi

    echo ""
    info "Environnement de bureau détecté : ${BOLD}${DESKTOP_ENV}${NC}"

    local should_install="o"
    if [ -e /dev/tty ]; then
        printf "${BOLD}Voulez-vous créer une entrée de menu et installer l'icône ? [O/n] ${NC}"
        read -r response < /dev/tty || response="o"
        if [ -n "$response" ]; then
            should_install="$response"
        fi
    fi

    case "$should_install" in
        [oOyY]|"")
            ;;
        *)
            info "Création de l'entrée de menu ignorée."
            return 0
            ;;
    esac

    # 1. Install Icon
    local icon_dir="$HOME/.local/share/icons/hicolor/scalable/apps"
    mkdir -p "$icon_dir"
    local icon_dest="${icon_dir}/spiritty.svg"

    if [ -f "${TMP_DIR}/spiritty.svg" ]; then
        cp "${TMP_DIR}/spiritty.svg" "$icon_dest"
    elif [ -f "${TMP_DIR}/assets/icons/spiritty.svg" ]; then
        cp "${TMP_DIR}/assets/icons/spiritty.svg" "$icon_dest"
    else
        info "Téléchargement de l'icône officielle..."
        curl -fsSL "https://raw.githubusercontent.com/${REPO}/${LATEST_TAG}/assets/icons/spiritty.svg" -o "$icon_dest" 2>/dev/null \
            || curl -fsSL "https://raw.githubusercontent.com/${REPO}/main/assets/icons/spiritty.svg" -o "$icon_dest" 2>/dev/null \
            || true
    fi

    # 2. Install Desktop Entry
    local app_dir="$HOME/.local/share/applications"
    mkdir -p "$app_dir"
    local desktop_dest="${app_dir}/spiritty.desktop"

    cat > "$desktop_dest" << EOF
[Desktop Entry]
Type=Application
Name=Spiritty
Comment=AI-powered split-screen terminal companion for sysadmins and power users
Exec=${INSTALL_DIR}/${BINARY_NAME}
Icon=spiritty
Terminal=true
Categories=System;TerminalEmulator;Development;
Keywords=terminal;ai;assistant;sysadmin;ssh;
EOF

    # 3. Refresh Caches
    if command -v update-desktop-database >/dev/null 2>&1; then
        update-desktop-database "$app_dir" 2>/dev/null || true
    fi
    if command -v gtk-update-icon-cache >/dev/null 2>&1; then
        gtk-update-icon-cache -f -t "$HOME/.local/share/icons/hicolor" 2>/dev/null || true
    fi
    if systemctl --user is-active dms.service >/dev/null 2>&1; then
        systemctl --user restart dms.service 2>/dev/null || true
    fi

    success "Entrée d'application et icône installées dans ${app_dir}/spiritty.desktop !"
}

main() {
    print_banner
    check_dependencies
    detect_platform
    get_latest_version
    get_install_dir
    install_binary
    check_path
    setup_desktop_entry

    echo ""
    echo -e "${GREEN}${BOLD}🎉 Installation terminée !${NC}"
    echo -e "Lancez simplement : ${CYAN}${BOLD}spiritty${NC}"
    echo ""
}

main "$@"
