#!/usr/bin/env bash
# ==============================================================================
#  👻 Spiritty — Official One-Line Installer
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
    echo -e "${CYAN}${BOLD}"
    echo "   _____       _      _ _   _         👻"
    echo "  / ____|     (_)    (_) | | |        "
    echo " | (___  _ __  _ _ __ _| |_| |_ _   _ "
    echo "  \___ \| '_ \| | '__| | __| __| | | |"
    echo "  ____) | |_) | | |  | | |_| |_| |_| |"
    echo " |_____/| .__/|_|_|  |_|\__|\__|\__, |"
    echo "        | |                      __/ |"
    echo "        |_|                     |___/ "
    echo -e "${NC}"
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
        LATEST_TAG="v0.2.0"
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

main() {
    print_banner
    check_dependencies
    detect_platform
    get_latest_version
    get_install_dir
    install_binary
    check_path

    echo ""
    echo -e "${GREEN}${BOLD}🎉 Installation terminée !${NC}"
    echo -e "Lancez simplement : ${CYAN}${BOLD}spiritty${NC}"
    echo ""
}

main "$@"
