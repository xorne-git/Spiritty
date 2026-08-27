# Changelog — Spiritty

Toutes les modifications notables de Spiritty sont documentées dans ce fichier.

## 📌 Convention

Ce journal suit le format [Keep a Changelog](https://keepachangelog.com/fr/1.1.0/).

- **Entre deux pushes** : les modifications s'accumulent dans la section **« Non publié »**.
- **Au moment d'un release (tag `vX.Y.Z`)** : la section « Non publié » est renommée en
  `## vX.Y.Z — YYYY-MM-DD` et une nouvelle section « Non publié » vide est ouverte au-dessus.
- Les catégories utilisées : `Ajouté` · `Modifié` · `Corrigé` · `Performance` · `Sécurité`.

---

## Non publié

_Rien pour l'instant._

---

## v0.4.5 — 2026-08-27

### Ajouté

- **Provider cloud Z.ai (GLM / Zhipu AI)** :
  - Nouveau type `ProviderType::Zai` avec affichage « Z.ai (GLM) », clé API `ZAI_API_KEY`
    (+ alias de secours `ZHIPU_API_KEY`) et base par défaut `https://api.z.ai/api/paas/v4`.
  - Détection automatique depuis la config sauvegardée via les alias `zai`, `z.ai`, `z_ai`,
    `z-ai`, `zhipu`, `glm`.
  - Listing dynamique des modèles géré pour l'URL de base se terminant par `/v4`.
  - Fenêtre de contexte auto-détectée à 131 072 tokens pour les modèles `glm*` / `*zai*`.
  - Modèles populaires pré-listés (de `glm-5.3` à `glm-4-flash`) ; pricing GLM intégré
    au cache local (`assets/pricing.json`).
  - Tests unitaires/intégration étendus (`config_test.rs`, `pricing_test.rs`).

- **`CHANGELOG.md`** : suivi structuré des modifications entre chaque push (cette convention).

### Corrigé — audits sécurité & bugs utilisateur (51d880d)

- 🔴 **Approbation accidentelle par Entrée-vide** : appuyer sur `Entrée` alors qu'une carte
  ⚡ DEMANDE D'AUTORISATION est affichée exécutait la commande en attente — même classée
  Risqué — car `""` était une phrase d'approbation naturelle. La carte affiche désormais
  `[oui/ok + ↵] Autoriser`.
- 🟠 **Proposition parasite `` `bash `` sans exécution** (screen rapporté) : un bloc ```bash
  contenant une ligne `bash` littérale produisait une proposition injection multi-lignes qui
  ouvrait un shell imbriqué au lieu d'exécuter la commande. Nouveau nettoyage
  `sanitize_proposed_command()` appliqué aux deux chemins d'extraction (fences markdown et
  `tool:run_command`) : lignes `bash/sh/zsh/shebang` supprimées, `exit`/`logout` orphelins en
  queue également (auraient tué le shell de l'utilisateur).
- 🟠 **Prose/token de sortie devenant « ⚡ COMMANDE #1 »** (screens « Mail queue is empty »,
  listes d'IP) : rejet des blocs non-tagués sentences capitales sans méta-shell, seuil prose
  abaissé de 7 à 5 mots.
- **Badge de risque divergent** : la carte ⚡ et les cartes `COMMANDE #N` utilisent toutes
  deux `safety::classify_command` comme source unique (avant : heuristiques ad-hoc montrant
  « Safe » vert sur des `kill -9`/`chmod`).

### Sécurité — taxonomie de risque à 4 niveaux (51d880d)

- Nouvelle classe `CommandRisk::Sudo` distincte de `Risky` : commandes élevées *non
  destructives* (`sudo ls/grep/cat/certbot certificates/systemctl status …`).
- Mapping d'auto-approbation revu : niveau `Sudo` ⇒ {Safe, Standard, Sudo} auto-approuvé ;
  le destructif (`rm`, restarts, installs paquets) reste derrière confirmation jusqu'au
  mode YOLO. Corrige le signalement utilisateur « je suis en approbation sudo et il me
  demande d'approuver une cmd read-only ».
- Ordre du classifieur inversé : la destructivité est évaluée avant l'élévation —
  `sudo rm/pacman -S/apt install/chmod` restent toujours `Risky`.
- `pacman/yay/paru` : détection tolère un préfixe `sudo/doas` (`sudo pacman -Syu` échappait
  à la règle).
- Clés API : `config.toml`, `hosts.json`, sessions JSON et cache pricing écrits en
  **0600** (avant : 0664 selon umask). La modale Ctrl+P ne réaffiche plus jamais une clé
  existante (champ vide = conserver ; frappe masquée en `••••` ; refs `ENV:` visibles).

### Performance

- **Capture PTY incrémentale** : fin du re-décodage UTF-8 du buffer entier à chaque chunk
  (O(n²) pendant les commandes verbeuses). Décodage avec carry multi-octets,
  détection de mot de passe sudo sur fenêtre arrière bornée (1 ko), recherche du sentinel
  OSC 777 avec watermark. Validé par tests (emoji splité en 4 chunks, octets invalides).
- **Thread UI débloqué** (règle AGENTS.md « ne JAMAIS bloquer ») :
  - Ctrl+V : lecture clipboard asynchrone fire-and-forget (garde anti-spam) au lieu d'un
    `recv_timeout(1500ms)` bloquant ; modales bornées à 1 s.
  - `$SHELL -l -c` (probe ENV) mémoïsé au niveau process : plus de spawn shell sous keydown
    lors des reloads de provider.
  - Fallback scan complet `/proc` du watcher mémoïsé 1,5 s par PID racine (au lieu de ×2
    toutes les ~360 ms sur raté transient du kernel).

### Corrigé — cycle de vie PTY (3afca40)

- Taper `exit` dans le panneau terminal figeait la TUI sur un PTY mort et laissait le shell
  en zombie. Désormais : thread reaper dédié (`child.wait()`) + notification d'exit
  single-shot (EOF lecteur ∪ wait child, garde `AtomicBool`) convertie en `AppEvent::PtyExit`
  → sortie propre avec sauvegarde de session.

### Infra & distribution

- `release.yml` : la LICENSE est maintenant incluse dans les tarballs publiés.
- `install.sh` : vérification **sha256** de l'archive téléchargée (échec dur si mismatch,
  warn-and-skip si le fichier checksum est absent).
- Mise à jour du fallback `LATEST_TAG` → `v0.4.5`.
- Repo entièrement normalisé `cargo fmt` (~530 hunks de dette effacés) ;
  clippy `-D warnings` propre ; suite de tests portée à **88 tests verts**
  (+8 : repro exacts des screens utilisateurs, matrice Sudo, décodeur UTF-8 fractionné,
  helpers char-boundary).

### Qualité interne (non visible)

- Fuite MCP corrigée : entrée `pending` retirée si l'écriture stdin échoue.
- Position 600 appliquée aussi bien au save config qu'aux writers hosts/sessions/pricing.

---

## v0.4.3 — 2026-08-26

### Corrigé

- Captures tronquées : drop des commentaires de heredoc en tête de proposition et rejet des
  blocs tabulaires comme propositions de commande.
- Fiabilisation de la capture silencieuse, intégration shell propre et jobs d'arrière-plan
  (v0.4.2) ; durcissement robustesse & sécurité (v0.4.1).
