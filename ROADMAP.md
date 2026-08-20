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

## 📌 Phase 4 : Contexte Système Avancé, Détection SSH & Auto-Remédiation (v0.4.0) [EN COURS ⏳]
*Objectif : Donner à l'agent une conscience aiguë de la machine hôte (locale ou serveur distant SSH) et la capacité de diagnostiquer les erreurs.*

- [x] **Détection Dynamique des Sessions SSH & Multi-Host Profiling :**
  - Surveillance non bloquante en temps réel de l'arbre de processus sous le PTY (`/proc/<pid>/...`).
  - Détection automatique des connexions `ssh`, `sftp`, `mosh-client`, `docker`, `podman`.
  - Cache persistant des profils serveurs dans `~/.config/spiritty/hosts.json` (OS, distribution, noyau, gestionnaires de paquets, init system).
  - Basculement instantané et automatique du *System Prompt* de l'IA lors des connexions/déconnexions SSH.
  - Scan d'hôte rapide d'un geste (`Alt + S`) pour profiler et mémoriser un nouveau serveur.
  - Indicateurs visuels d'en-tête et de statut (`🌐 SSH: user@host (Distro)`).
- [x] **Extracteur de Contexte Système Local :**
  - Détection automatique de la distribution Linux (Arch, CachyOS, Ubuntu, Debian, Fedora, Alpine) ou macOS.
  - Détection des gestionnaires de paquets installés (`apt`, `pacman`, `dnf`, `brew`, `nix`, `cargo`, `yay`, `paru`, `flatpak`, `snap`).
  - Capture du shell actif, de l'émulateur de terminal hôte et de l'environnement graphique (`Wayland`/`X11`/`niri`/`hyprland`).
- [ ] **Capture & Diagnostic d'Erreur :**
  - Détection des codes de retour non nuls (`$? != 0`) et des messages d'erreur stderr dans le terminal droit.
  - Proposition proactive de l'agent : *"La commande a échoué avec l'erreur X. Souhaitez-vous que je tente de résoudre le problème ?"*.

---

## 📌 Phase 5 : Finitions, Performances & Distribution v1.0.0 (v1.0.0)
*Objectif : Produire un binaire ultra-rapide, stable et prêt pour une adoption large.*

- [ ] **Ergonomie & Thèmes :**
  - Thèmes prédéfinis (Catppuccin, Nord, Tokyo Night, Gruvbox, Monokai).
  - Redimensionnement dynamique de la séparation gauche/droite (slider à la souris ou raccourci clavier).
- [ ] **Tests de Robustesse :**
  - Gestion des applications ncurses interactives dans le PTY droit (`vim`, `nano`, `htop`, `fzf`).
  - Gestion propre des signaux `SIGINT`, `SIGTERM`, `SIGHUP`.
- [ ] **Packaging & Distribution :**
  - Binaire statique musl pour Linux (`x86_64`, `aarch64`).
  - Binaire universel macOS.
  - Dépôts : `cargo install spiritty`, PKGBUILD pour Arch Linux (AUR), Homebrew tap.
