#!/usr/bin/env bash
# ==============================================================================
#  Spiritty — Omarchy agents-panel integration (idempotent installer)
# ==============================================================================
#  Installs the two artifacts Omarchy needs to render Spiritty as a
#  first-class agent in its agents panel:
#
#    1. symlink  $OMARCHY_PATH/bin/omarchy-agent-usage-spiritty
#         ->  <repo>/tools/omarchy-agent-usage-spiritty
#    2. copy of  <repo>/assets/icons/spiritty.svg
#         ->  $OMARCHY_PATH/shell/plugins/agents/assets/spiritty.svg
#
#  Safe to re-run after every Omarchy upgrade. It is a no-op when the two
#  artifacts are already correct (symlink target matches, SVG bytes identical).
# ==============================================================================
set -euo pipefail

# --- layout -------------------------------------------------------------------
SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "$SCRIPT_DIR/.." && pwd)"

COLLECTOR_SRC="$REPO_ROOT/tools/omarchy-agent-usage-spiritty"
SVG_SRC="$REPO_ROOT/assets/icons/spiritty.svg"

OMARCHY_PATH="${OMARCHY_PATH:-/usr/share/omarchy}"
BIN_DIR="$OMARCHY_PATH/bin"
ASSETS_DIR="$OMARCHY_PATH/shell/plugins/agents/assets"
UPDATE_BIN="$BIN_DIR/omarchy-agent-usage-update"

COLLECTOR_DEST="$BIN_DIR/omarchy-agent-usage-spiritty"
SVG_DEST="$ASSETS_DIR/spiritty.svg"

# --- pretty -------------------------------------------------------------------
if [[ -t 1 ]]; then
  BOLD=$'\e[1m'; GREEN=$'\e[32m'; YELLOW=$'\e[33m'; RED=$'\e[31m'; DIM=$'\e[2m'; RESET=$'\e[0m'
else
  BOLD=""; GREEN=""; YELLOW=""; RED=""; DIM=""; RESET=""
fi

info() { printf '%s\n' "${DIM}•${RESET} $*"; }
ok()   { printf '%s\n' "${GREEN}✓${RESET} $*"; }
skip() { printf '%s\n' "${YELLOW}↷${RESET} $*"; }
warn() { printf '%s\n' "${YELLOW}!${RESET} $*" >&2; }
die()  { printf '%s\n' "${RED}✗${RESET} $*" >&2; exit 1; }

# --- preflight ----------------------------------------------------------------
[[ -x "$COLLECTOR_SRC" ]] || die "Collector introuvable ou non exécutable : $COLLECTOR_SRC"
[[ -r "$SVG_SRC" ]]       || die "SVG source introuvable : $SVG_SRC"
[[ -d "$OMARCHY_PATH" ]]  || die "Omarchy introuvable ($OMARCHY_PATH). Définis OMARCHY_PATH."
command -v jq   >/dev/null 2>&1 || die "jq requis pour valider le JSON du collector."
command -v cmp  >/dev/null 2>&1 || die "cmp requis (paquet diffutils)."

# --- sudo escalation (only if needed) -----------------------------------------
SUDO=""
if [[ ! -w "$BIN_DIR" || ! -w "$ASSETS_DIR" ]]; then
  command -v sudo >/dev/null 2>&1 || die "sudo requis pour écrire dans $OMARCHY_PATH."
  info "Élévation de privilèges requise (sudo)…"
  sudo -v || die "Échec de l'authentification sudo."
  SUDO="sudo"
fi

# --- 1. collector symlink -----------------------------------------------------
info "Symlink du collector…"
if [[ -L "$COLLECTOR_DEST" ]] && [[ "$(readlink -f "$COLLECTOR_DEST")" == "$COLLECTOR_SRC" ]]; then
  skip "Déjà en place : $COLLECTOR_DEST"
else
  $SUDO ln -sfn "$COLLECTOR_SRC" "$COLLECTOR_DEST"
  ok "Symlink : $COLLECTOR_DEST → $COLLECTOR_SRC"
fi

# --- 2. panel icon ------------------------------------------------------------
info "Icône du panneau…"
if [[ -r "$SVG_DEST" ]] && cmp -s "$SVG_SRC" "$SVG_DEST"; then
  skip "Déjà à jour : $SVG_DEST"
else
  $SUDO install -m 0644 "$SVG_SRC" "$SVG_DEST"
  ok "Icône : $SVG_DEST"
fi

# --- 3. smoke test ------------------------------------------------------------
info "Auto-test du collector…"
JSON="$("$COLLECTOR_DEST" --force 2>/dev/null)" \
  || die "Le collector a échoué à produire un JSON."
printf '%s' "$JSON" | jq -e . >/dev/null \
  || die "Le collector n'a pas produit un JSON valide."
ok "JSON valide ($(printf '%s' "$JSON" | wc -c | tr -d ' ') octets)."

# --- 4. refresh the panel -----------------------------------------------------
if [[ -x "$UPDATE_BIN" ]]; then
  if "$UPDATE_BIN" >/dev/null 2>&1; then
    ok "Panneau rafraîchi via $(basename "$UPDATE_BIN")."
  else
    warn "$(basename "$UPDATE_BIN") a échoué — relance-le manuellement."
  fi
else
  warn "omarchy-agent-usage-update introuvable — le panneau se rafraîchira à son prochain cycle."
fi

echo
ok "${BOLD}Intégration Omarchy de Spiritty opérationnelle.${RESET}"
