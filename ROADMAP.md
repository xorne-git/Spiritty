# Feuille de Route (Roadmap) — Spiritty

Ce document définit les étapes clés du développement de **Spiritty**, du prototype initial jusqu'à la version 1.0.

---

## 🎯 Vue Globale des Jalons

```
[Phase 1: Fondations TUI & PTY] ──> [Phase 2: Moteur d'Agent & LLM] ──> [Phase 3: Human-in-the-Loop & Actions]
                                                                               │
[Phase 5: Release v1.0 & Distrib] <── [Phase 4: Contexte Système & Polishing] <──┘
```

---

## 📌 Phase 1 : Fondations TUI & Terminal PTY (v0.1.0) [TERMINÉ ✅]
*Objectif : Avoir une application TUI fluide avec un split-screen fonctionnel et un shell interactif natif dans le panneau droit.*

- [x] **Initialisation du projet Cargo :**
  - Configuration du `Cargo.toml` avec dépendances (`ratatui`, `crossterm`, `tokio`, `portable-pty`, `vt100`, `anyhow`, `serde`).
- [x] **Boucle d'événements & Layout de base :**
  - Layout en écran scindé 40/60 horizontal.
  - Gestion du focus clavier : Bascule rapide (`Ctrl+A`) entre Chat et Terminal.
  - Barre d'état (Header & Footer) avec raccourcis et statut du système.
- [x] **Intégration du PTY dans Ratatui :**
  - Spawn du shell par défaut (`$SHELL`) via `portable-pty`.
  - Capture et parsing des octets ANSI/VT100 via `vt100`.
  - Rendu du buffer virtuel dans les cellules de `ratatui::buffer::Buffer`.
  - Transmission des frappes clavier au PTY maître en mode raw.
  - Gestion dynamique du redimensionnement de la fenêtre (`SIGWINCH` / `pty.resize`).

---

## 📌 Phase 2 : Moteur d'Agent & Intégration LLM (v0.2.0) [TERMINÉ ✅]
*Objectif : Connecter un LLM au panneau gauche avec streaming des réponses, configuration multi-providers (Ollama, LM Studio, Gemini, Grok, DeepSeek, OpenAI, Claude) et modales interactives.*

- [x] **Panneau de Chat interactif & Streaming :**
  - Zone de saisie multi-lignes, historique des messages, curseur matériel.
  - Streaming asynchrone sans bloquer le shell interactif PTY.
  - Indicateur visuel d'état (`👻 Spiritty réfléchit...`).
- [x] **Fournisseurs LLM (Multi-Providers) :**
  - Client Ollama (Modèles locaux comme `qwen2.5-coder`, `deepseek-r1`).
  - Client LM Studio (Serveur local OpenAI-compatible).
  - Client Grok / xAI (`api.x.ai/v1`).
  - Client Google Gemini (REST SSE streaming).
  - Client DeepSeek & OpenAI.
  - Client Anthropic Claude.
- [x] **Gestion de la Configuration & Modales :**
  - Fichier de configuration TOML (`~/.config/spiritty/config.toml`).
  - Modale interactive de configuration (`Ctrl+P`) pour changer de provider/modèle/clé.
  - Modale d'aide aux raccourcis (`F1`).
- [x] **Moteur d'Internationalisation (i18n) :**
  - Détection automatique de la langue système (`$LANG`) et support Français/Anglais.
  - Surcharge de la langue dans la configuration (`language = "fr"`).
  - Traduction de toutes les modales, bandeaux d'aide, statuts et system prompts.

---

## 📌 Phase 3 : Validation Humaine, Exécution de Commandes & Sessions (v0.3.0) [TERMINÉ ✅]
*Objectif : Permettre à l'agent de proposer des commandes et à l'utilisateur de les exécuter d'un geste dans le terminal droit, avec persistance et compactage de sessions.*

- [x] **Composant "Command Proposal Card" :**
  - Détection automatique des blocs de commandes proposés par l'IA et filtrage des explications.
  - Cartes d'actions interactives multi-propositions (`Alt + 1..9`).
- [x] **Actions Clavier & Exécution Live PTY :**
  - `[Enter]` : Envoi direct au modèle ou injection dans le PTY.
  - `[Alt + N]` : Exécution de la proposition N avec capture et analyse du résultat.
- [x] **Gestionnaire de Sessions & Compactage de Contexte :**
  - Stockage JSON structuré dans `~/.config/spiritty/sessions/`.
  - Modale interactive de navigation de sessions (`Ctrl + H`) avec rechargement, création (`Ctrl + N`), suppression et compactage manuel.
  - Compactage automatique intelligent des anciens tours de dialogue pour préserver les tokens.
  - Auto-sauvegarde systématique de la session en cours à la fermeture de l'application.

---

## 📌 Phase 4 : Contexte Système Avancé, Détection SSH & Auto-Remédiation (v0.4.0) [TERMINÉ ✅]
*Objectif : Donner à l'agent une conscience aiguë de la machine hôte (locale ou serveur distant SSH), une exécution silencieuse sans pollution de terminal et un shell 100% interactif en continu.*

- [x] **Détection Dynamique des Sessions SSH & Multi-Host Profiling :**
  - Surveillance non bloquante en temps réel de l'arbre de processus sous le PTY (`/proc/<pid>/...`).
  - Détection automatique des connexions `ssh`, `sftp`, `mosh-client`, `docker`, `podman`.
  - Cache persistant des profils serveurs dans `~/.config/spiritty/hosts.json` (OS, distribution, noyau, gestionnaires de paquets, init system).
  - Basculement instantané et automatique du *System Prompt* de l'IA lors des connexions/déconnexions SSH.
  - Indicateurs visuels d'en-tête et de statut épurés (`🌐 SSH: user@host (Distro)`).
- [x] **Exécution Silencieuse et Shell Interactif en Continu :**
  - Exécution 100% propre sans aucune sentinelle visible (`printf "\033]..."`) dans le terminal PTY.
  - Saisie shell continue et non-bloquante pendant la réflexion et le streaming du modèle.
  - Bascule automatique du focus sur le shell à la soumission du prompt pour une ergonomie optimale.
  - Prévention de la pollution de l'historique shell (espace initial pour Fish, Bash, Zsh).
- [x] **Éditeur de Prompt Multi-Lignes & Auto-Réparation Markdown :**
  - Passage à la ligne fluide via `Shift + Enter`, `Alt + Enter`, `Ctrl + Enter` et `Ctrl + J`.
  - Auto-réparation à la volée des blocs de code fermés prématurément par les LLMs.
  - Filtrage des faux blocs de commandes (flèches de transition, descriptions).
- [x] **Extracteur de Contexte Système Local :**
  - Détection automatique de la distribution Linux (Arch, CachyOS, Ubuntu, Debian, Fedora, Alpine) ou macOS.
  - Détection des gestionnaires de paquets installés (`apt`, `pacman`, `dnf`, `brew`, `nix`, `cargo`, `yay`, `paru`, `flatpak`, `snap`).
  - Capture du shell actif, de l'émulateur de terminal hôte et de l'environnement graphique (`Wayland`/`X11`/`niri`/`hyprland`).
- [x] **Capture & Diagnostic d'Erreur Proactif (`Alt + D`) :**
  - Détection automatique des commandes échouées et erreurs d'exécution dans le PTY.
  - Carte d'alerte et remédiation automatique en un raccourci (`Alt + D`).
- [x] **Indicateur de Répertoire Courant (PWD) & Branche Git :**
  - Affichage instantané du dossier actif et de la branche Git dans l'en-tête du terminal.
  - Injection dynamique du PWD et de la branche Git dans le contexte système de l'agent.
- [x] **Export de Session en Rapport Markdown (`Ctrl + E`) :**
  - Génération en 1 touche d'un rapport structuré avec horodatage, métadonnées machine, historique des prompts et commandes dans `~/.config/spiritty/exports/`.
- [x] **Gestionnaire de Serveurs SSH Favoris (`Ctrl + B`) :**
  - Modale interactive de favoris SSH avec ajout rapide, recherche, étoiles de favoris et connexion en 1 touche.
- [x] **Recherche en Temps Réel dans l'Historique de Chat (`Ctrl + F`) :**
  - Barre de recherche avec surbrillance dynamique et navigation rapide entre occurrences (`Enter` / `Shift + Enter`).

---

## 📌 Phase 5 : Protocoles Avancés, Métriques Précises & Distribution v1.0.0 (v1.0.0) [EN COURS 🚀]
*Objectif : Intégrer l'écosystème MCP, garantir une précision métrique absolue des tokens et du débit, et produire un binaire ultra-rapide et stable.*

- [x] **Support du Protocole MCP (Model Context Protocol) & Modale TUI Dédiée (`Ctrl + M`) :**
  - Moteur client MCP stdio asynchrone (JSON-RPC 2.0) avec initialisation, négociation de capacités et découverte dynamique des outils (`tools/list`).
  - Découverte et exposition dynamique des outils MCP dans le prompt système de l'agent (`mcp:<server>:<tool>`).
  - Interception et exécution asynchrone des appels d'outils MCP par l'agent (`ToolInvocation::McpCall`).
  - Modale TUI interactive (`Ctrl + M`) : liste des serveurs, inspecteur d'outils, activation/désactivation en 1 touche (`Espace`), rechargement (`R`), ajout (`A`) et suppression (`D`).
- [x] **Calcul Précis des Tokens, du Débit (tokens/s) & Registre Dynamique des Coûts :**
  - Exploitation des métriques natives renvoyées par les APIs LLM (`eval_count`/`eval_duration` dans Ollama, `stream_options.include_usage` dans OpenAI/DeepSeek/Grok, `message_delta.usage` dans Anthropic, `usageMetadata` dans Gemini).
  - Élimination des artefacts de calcul du débit : chronomètre de streaming démarré dès le 1er chunk utile, déduction des latences réseau et pauses d'exécution d'outils.
  - Estimation et affichage en temps réel du coût de session en dollars (`💵 $0.0042`) dans le footer et les modales de session pour les modèles cloud.
  - **Registre dynamique des tarifs LLM (`src/pricing/`) :** support des surcharges personnalisées dans `~/.config/spiritty/config.toml` (`[pricing."nom_modele"]`), persistance du cache local dans `~/.config/spiritty/pricing.json`, et mise à jour/synchronisation en 1 touche depuis Internet (`Ctrl + P` puis `[U]`).
- [x] **Modal Toast Non-Intrusif & Diagnostic Proactif Ciblé (`Alt + D`) :**
  - Notification d'erreur shell sous forme de toast flottant élégant en bas à droite du panneau terminal avec bordure arrondie (`Alt + D` Diagnostiquer / `Alt + X` Fermer).
  - Restriction stricte du diagnostic proactif aux seules commandes tapées manuellement par l'utilisateur dans le shell interactif.
  - Respect absolu de la propreté du panneau de chat : 0 pollution lors des erreurs de commandes manuelles.
- [x] **Ergonomie & Thèmes :**
  - 7 thèmes prédéfinis avec dégradés verticaux et palettes coordonnées : *Spiritty Dark*, *Catppuccin Mocha*, *Tokyo Night*, *Nord Arctic*, *Gruvbox Dark*, *Dracula*, *Monokai Pro*.
  - Sélecteur interactif de thème dans la modale `Ctrl + P` avec prévisualisation dynamique instantanée.
  - Redimensionnement fluide de la séparation gauche/droite à la souris (glisser-déposer) ou au clavier (`Alt + ←` / `Alt + →`).
  - Persistance automatique de la taille des panneaux (`split_ratio`), de la configuration MCP et du thème actif dans `~/.config/spiritty/config.toml`.
  - Diagnostics d'erreur explicites lors de pannes de connexion LLM (serveurs locaux éteints, timeout, erreurs réseau).
- [x] **Hardening v0.4.5 — Sécurité, Perf & Fiabilité (audit complet) :** *(détail complet dans [CHANGELOG.md](CHANGELOG.md))*
  - Sécurité human-in-the-loop : faille « Entrée-vide approuve la commande en attente » fermée ; taxonomie de risque à 4 niveaux (`Safe / Standard / Sudo / Risky`) avec auto-approbation `Sudo` couvrant les commandes élevées read-only.
  - Hygiène des secrets : `config.toml`/`hosts.json`/sessions/pricing écrits en 0600, clé API jamais réaffichée dans la modale (masquage + conservation si vide).
  - Cycle de vie PTY : sortie propre sur `exit` du shell (`PtyExit` + reaper thread), plus aucun zombie ni panneau figé.
  - Thread UI jamais bloqué : Ctrl+V asynchrone, probe ENV mémoïsée, scan `/proc` borné (1,5 s TTL).
  - Capture PTY incrémentale : fin du O(n²) sur commandes verbeuses (décodage UTF-8 avec carry, fenêtres bornées, watermark sentinel).
  - Nettoyage des propositions LLM : lignes interpréteur parasites (`bash`, shebangs, `exit` orphelins) supprimées ; prose/tabulations de sortie ne deviennent plus des cartes ⚡.
  - Provider Z.ai (GLM/Zhipu) ajouté avec pricing intégré et détection d'alias.
- [x] **Hardening v0.5.0 — Boucle d'outils fiable & UI indestructible :** *(détail complet dans [CHANGELOG.md](CHANGELOG.md))*
  - Boucle d'outils textuels réellement exécutée à nouveau (markup DSML hybride DeepSeek/GLM parsé) ; échos corrompus des redraws SSH reconnus et retirés — plus d'hallucinations « saboteur » nourries par l'écho.
  - Fiabilité : écritures PTY off-thread (plus aucun freeze UI sur SSH calé), capture plafonnée à 1 Mio avec nettoyage paresseux + avis de troncature, restauration terminal sur SIGTERM/SIGINT/SIGHUP.
  - UX : timer « 💭 Deep thinking… » ré-armé à chaque segment de réflexion, plus de double prompt `Alt+N`/`F10` sur une même commande (snippet inerté + Alt+N bloqué pendant un consentement ou une capture).
- [ ] **Tests de Robustesse :**
  - Gestion des applications ncurses interactives dans le PTY droit (`vim`, `nano`, `htop`, `fzf`).
  - [x] Gestion propre des signaux `SIGINT`, `SIGTERM`, `SIGHUP` (restauration complète du terminal + sortie `128+signal`, même UI figée).
- [x] **Packaging & Distribution Automatisée :**
  - Script d'installation universel one-line `install.sh` (`curl -fsSL https://raw.githubusercontent.com/xorne-git/Spiritty/main/install.sh | bash`) avec détection automatique de l'OS et de l'architecture (`x86_64`, `aarch64`, macOS).
  - Pipeline de publication automatisé GitHub Actions multi-cibles (`release.yml`) générant les binaires allégés (`strip`) et les archives tarball sur chaque tag `v*`.
  - Binaire statique et universel prêt pour `cargo install`, AUR et Homebrew.
