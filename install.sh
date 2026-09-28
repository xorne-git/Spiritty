#!/usr/bin/env bash
# ==============================================================================
#  🧞 Spiritty — Official One-Line Installer
#  Usage:
#    curl -fsSL https://raw.githubusercontent.com/xorne-git/Spiritty/main/install.sh | bash
#    bash install.sh --voice-only   # (re)configure the local voice input only
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
        LATEST_TAG="v0.10.0"
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

    # 1. Install Icons (multi-res PNG & pixmaps)
    local icon_512_dir="$HOME/.local/share/icons/hicolor/512x512/apps"
    local pixmaps_dir="$HOME/.local/share/pixmaps"
    mkdir -p "$icon_512_dir" "$pixmaps_dir"
    local icon_png_dest="${icon_512_dir}/spiritty.png"
    local pixmaps_png_dest="${pixmaps_dir}/spiritty.png"

    # Remove stale old SVG icon if present so it does not hijack PNG icon resolution in Qt/DMS
    rm -f "$HOME/.local/share/icons/hicolor/scalable/apps/spiritty.svg" 2>/dev/null || true

    local src_png=""
    if [ -f "${TMP_DIR}/spiritty.png" ]; then
        src_png="${TMP_DIR}/spiritty.png"
    elif [ -f "${TMP_DIR}/assets/icons/spiritty.png" ]; then
        src_png="${TMP_DIR}/assets/icons/spiritty.png"
    elif [ -f "assets/icons/spiritty.png" ]; then
        src_png="assets/icons/spiritty.png"
    fi

    if [ -n "$src_png" ] && [ -f "$src_png" ]; then
        cp "$src_png" "$icon_png_dest"
        cp "$src_png" "$pixmaps_png_dest"
    else
        info "Téléchargement de l'icône officielle..."
        curl -fsSL "https://raw.githubusercontent.com/${REPO}/main/assets/icons/spiritty.png" -o "$icon_png_dest" 2>/dev/null \
            || true
        [ -f "$icon_png_dest" ] && cp "$icon_png_dest" "$pixmaps_png_dest" 2>/dev/null || true
    fi

    # Install multi-size icons if convert/magick or python is present
    if command -v magick >/dev/null 2>&1 && [ -f "$icon_png_dest" ]; then
        for sz in 16 24 32 48 64 96 128 256; do
            local sz_dir="$HOME/.local/share/icons/hicolor/${sz}x${sz}/apps"
            mkdir -p "$sz_dir"
            magick "$icon_png_dest" -resize "${sz}x${sz}" "${sz_dir}/spiritty.png" 2>/dev/null || true
        done
    elif command -v convert >/dev/null 2>&1 && [ -f "$icon_png_dest" ]; then
        for sz in 16 24 32 48 64 96 128 256; do
            local sz_dir="$HOME/.local/share/icons/hicolor/${sz}x${sz}/apps"
            mkdir -p "$sz_dir"
            convert "$icon_png_dest" -resize "${sz}x${sz}" "${sz_dir}/spiritty.png" 2>/dev/null || true
        done
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
    if systemctl --user is-active dsearch.service >/dev/null 2>&1; then
        systemctl --user restart dsearch.service 2>/dev/null || true
    fi
    if systemctl --user is-active dms.service >/dev/null 2>&1; then
        systemctl --user restart dms.service 2>/dev/null || true
    fi

    success "Entrée d'application et icône installées dans ${app_dir}/spiritty.desktop !"
}

# Ensures the tools needed to build whisper.cpp are present (git, cmake, make and a
# C++ compiler), offering to install the missing ones through the detected package
# manager. Returns 0 when everything is available, 1 otherwise.
ensure_build_tools() {
    local missing=()
    command -v git >/dev/null 2>&1 || missing+=(git)
    command -v cmake >/dev/null 2>&1 || missing+=(cmake)
    command -v make >/dev/null 2>&1 || missing+=(make)
    if ! command -v c++ >/dev/null 2>&1 \
        && ! command -v g++ >/dev/null 2>&1 \
        && ! command -v clang++ >/dev/null 2>&1; then
        missing+=(g++)
    fi

    if [ "${#missing[@]}" -eq 0 ]; then
        return 0
    fi

    warn "Outils de compilation manquants : ${missing[*]}"

    local pm="" pkgs=""
    if command -v pacman >/dev/null 2>&1; then
        pm="sudo pacman -S --needed --noconfirm"; pkgs="git cmake make gcc"
    elif command -v apt-get >/dev/null 2>&1; then
        pm="sudo apt-get install -y"; pkgs="git cmake build-essential"
    elif command -v dnf >/dev/null 2>&1; then
        pm="sudo dnf install -y"; pkgs="git cmake make gcc-c++"
    elif command -v zypper >/dev/null 2>&1; then
        pm="sudo zypper install -y"; pkgs="git cmake make gcc-c++"
    elif command -v apk >/dev/null 2>&1; then
        pm="sudo apk add"; pkgs="git cmake make g++"
    fi

    if [ -z "$pm" ]; then
        warn "Aucun gestionnaire de paquets reconnu : installez ${missing[*]} manuellement, puis relancez."
        return 1
    fi

    echo -e "${GRAY}   Commande proposée : ${NC}${BOLD}${pm} ${pkgs}${NC}"
    if [ -e /dev/tty ]; then
        printf "${BOLD}Installer ces outils maintenant ? [O/n] ${NC}"
        local ans="o"
        read -r ans < /dev/tty || ans="o"
        case "$ans" in
            [oOyY]|"") ;;
            *) warn "Installation des outils ignorée : compilation impossible."; return 1 ;;
        esac
    else
        return 1
    fi

    info "Installation des outils de compilation..."
    # shellcheck disable=SC2086
    $pm $pkgs >/dev/null 2>&1 || { warn "Échec de l'installation des outils de compilation."; return 1; }
    success "Outils de compilation installés."
    return 0
}

# --- 8. Optional 100% Local Voice Input (whisper.cpp + model) ---
#
# Adds the offline dictation prerequisites: an audio recorder check, a local
# `whisper-cli` (from PATH, an existing build, or a from-source build), a GGML
# model download, and the `[voice]` section of the config. Never blocks the
# install: every step degrades to a warning.
setup_voice_input() {
    # Non-interactive (piped/CI) installs skip the ~500 MB model download.
    if [ ! -e /dev/tty ]; then
        return 0
    fi

    echo ""
    printf "${BOLD}Installer l'entrée vocale locale 100%% hors-ligne (whisper.cpp + modèle) ? [O/n] ${NC}"
    local response="o"
    read -r response < /dev/tty || response="o"
    case "$response" in
        [oOyY]|"")
            ;;
        *)
            info "Entrée vocale ignorée. Activez-la plus tard via la section [voice] de la configuration."
            return 0
            ;;
    esac

    local config_dir="${XDG_CONFIG_HOME:-$HOME/.config}/spiritty"
    local models_dir="${config_dir}/models"
    local model_size="${SPIRITTY_VOICE_MODEL:-small}"
    local model_path="${models_dir}/ggml-${model_size}.bin"
    local whisper_dir="${SPIRITTY_WHISPER_DIR:-$HOME/.local/opt/whisper.cpp}"
    local whisper_bin="${SPIRITTY_WHISPER_BIN:-}"

    # Spiritty rewrites config.toml from memory on some actions; warn if it is running so
    # the user does not lose the [voice] patch below.
    if pgrep -x spiritty >/dev/null 2>&1; then
        warn "Spiritty est en cours d'exécution : il peut réécrire config.toml et annuler ces réglages."
        echo -e "${GRAY}   Ferme Spiritty, puis relance 'bash install.sh --voice-only' si besoin.${NC}"
    fi

    # 1. Audio recorder (best-effort: never fatal).
    if ! command -v arecord >/dev/null 2>&1 \
        && ! command -v ffmpeg >/dev/null 2>&1 \
        && ! command -v sox >/dev/null 2>&1; then
        warn "Aucun enregistreur audio trouvé (arecord, ffmpeg ou sox)."
        echo -e "${GRAY}   Installez-en un (ex. 'alsa-utils' pour arecord) pour capturer le micro.${NC}"
    else
        success "Enregistreur audio détecté."
    fi

    # 2. Locate or build whisper-cli.
    if [ -n "$whisper_bin" ] && [ -x "$whisper_bin" ]; then
        :
    elif command -v whisper-cli >/dev/null 2>&1; then
        whisper_bin="$(command -v whisper-cli)"
    elif [ -x "${whisper_dir}/build/bin/whisper-cli" ]; then
        whisper_bin="${whisper_dir}/build/bin/whisper-cli"
    else
        if ! ensure_build_tools; then
            echo -e "${GRAY}   Installez les outils ci-dessus (ou un paquet 'whisper-cpp') puis relancez.${NC}"
            return 0
        fi

        info "Compilation locale de whisper.cpp dans ${whisper_dir} (quelques minutes)..."
        if [ ! -d "${whisper_dir}/.git" ]; then
            git clone --depth 1 https://github.com/ggml-org/whisper.cpp "$whisper_dir" \
                || { warn "Clonage de whisper.cpp échoué."; return 0; }
        fi
        local build_log="${whisper_dir}/build.log"
        cmake -S "$whisper_dir" -B "${whisper_dir}/build" -DCMAKE_BUILD_TYPE=Release \
            -DWHISPER_BUILD_TESTS=OFF -DWHISPER_BUILD_SERVER=OFF >"$build_log" 2>&1 \
            || { warn "Configuration CMake de whisper.cpp échouée (voir ${build_log})."; return 0; }
        cmake --build "${whisper_dir}/build" -j"$(getconf _NPROCESSORS_ONLN 2>/dev/null || echo 2)" >>"$build_log" 2>&1 \
            || { warn "Compilation de whisper.cpp échouée (voir ${build_log})."; return 0; }
        whisper_bin="${whisper_dir}/build/bin/whisper-cli"
    fi

    if [ ! -x "$whisper_bin" ]; then
        warn "Binaire whisper-cli introuvable après installation."
        return 0
    fi
    success "whisper-cli prêt : ${whisper_bin}"

    # 3. Download the GGML model (skipped when already present).
    mkdir -p "$models_dir"
    if [ -s "$model_path" ]; then
        success "Modèle déjà présent : ${model_path}"
    else
        info "Téléchargement du modèle Whisper '${model_size}' (~500 Mo)..."
        local model_url="https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-${model_size}.bin"
        if command -v curl >/dev/null 2>&1; then
            curl -fL --retry 3 -o "$model_path" "$model_url" \
                || { warn "Téléchargement du modèle échoué."; rm -f "$model_path"; return 0; }
        else
            wget -q -O "$model_path" "$model_url" \
                || { warn "Téléchargement du modèle échoué."; rm -f "$model_path"; return 0; }
        fi
        success "Modèle installé : ${model_path}"
    fi

    # 4. Enable [voice] in the config (patch in place, or create a minimal one).
    local config_file="${config_dir}/config.toml"
    set_voice_config "$config_file" "$whisper_bin" "$model_path"
    success "Entrée vocale activée dans ${config_file}"

    echo ""
    echo -e "${GRAY}   Dans Spiritty : ${NC}${BOLD}F7${NC}${GRAY} = dictée continue (pause = validation), ${NC}${BOLD}F8${NC}${GRAY} = segment manuel.${NC}"
    echo -e "${GRAY}   Pour l'envoi automatique, réglez ${NC}${BOLD}auto_submit = true${NC}${GRAY} dans [voice].${NC}"
}

# Patches `[voice]` in a TOML file without disturbing the rest of the config:
# replaces `enabled`, `whisper_bin` and `model_path` when present, inserts the
# missing ones (or the whole section when absent). Leaves every other key intact.
set_voice_config() {
    local config_file="$1" whisper_bin="$2" model_path="$3"
    mkdir -p "$(dirname "$config_file")"

    if [ ! -f "$config_file" ]; then
        cat > "$config_file" <<EOF
[voice]
enabled = true
whisper_bin = "$whisper_bin"
model_path = "$model_path"
EOF
        chmod 600 "$config_file" 2>/dev/null || true
        return 0
    fi

    local tmp="${config_file}.tmp"
    if awk -v wb="$whisper_bin" -v mp="$model_path" '
        function flush_missing() {
            if (!done_en) { print "enabled = true"; done_en = 1 }
            if (!done_wb) { print "whisper_bin = \"" wb "\""; done_wb = 1 }
            if (!done_mp) { print "model_path = \"" mp "\""; done_mp = 1 }
        }
        BEGIN { in_voice = 0; found_voice = 0 }
        /^\[/ {
            if (in_voice) flush_missing()
            in_voice = ($0 ~ /^\[voice\][[:space:]]*$/)
            if (in_voice) found_voice = 1
            print
            next
        }
        {
            if (in_voice) {
                if ($0 ~ /^[[:space:]]*enabled[[:space:]]*=/) { print "enabled = true"; done_en = 1; next }
                if ($0 ~ /^[[:space:]]*whisper_bin[[:space:]]*=/) { print "whisper_bin = \"" wb "\""; done_wb = 1; next }
                if ($0 ~ /^[[:space:]]*model_path[[:space:]]*=/) { print "model_path = \"" mp "\""; done_mp = 1; next }
            }
            print
        }
        END {
            if (in_voice) flush_missing()
            if (!found_voice) {
                print ""
                print "[voice]"
                print "enabled = true"
                print "whisper_bin = \"" wb "\""
                print "model_path = \"" mp "\""
            }
        }
    ' "$config_file" > "$tmp"; then
        mv "$tmp" "$config_file"
    else
        rm -f "$tmp"
        warn "Impossible de mettre à jour ${config_file}; configurez [voice] manuellement."
    fi
    chmod 600 "$config_file" 2>/dev/null || true
    return 0
}

main() {
    local voice_only=0
    for arg in "$@"; do
        case "$arg" in
            --voice-only|--voice) voice_only=1 ;;
        esac
    done

    print_banner
    check_dependencies

    # `--voice-only`: (re)configure the local voice input without touching the binary —
    # useful to retry after installing a missing build tool (e.g. cmake).
    if [ "$voice_only" -eq 1 ]; then
        setup_voice_input
        echo ""
        echo -e "${GREEN}${BOLD}🎉 Entrée vocale configurée.${NC}"
        echo ""
        return 0
    fi

    detect_platform
    get_latest_version
    get_install_dir
    install_binary
    check_path
    setup_desktop_entry
    setup_voice_input

    echo ""
    echo -e "${GREEN}${BOLD}🎉 Installation terminée !${NC}"
    echo -e "Lancez simplement : ${CYAN}${BOLD}spiritty${NC}"
    echo ""
}

main "$@"
