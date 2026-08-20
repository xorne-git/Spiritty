# AGENTS.md — Directives de Développement pour Spiritty

Ce document définit les standards d'ingénierie, l'architecture du code et les règles que tout agent IA ou développeur doit respecter lorsqu'il contribue au projet **Spiritty**.

---

## 🎯 Vision du Projet
Spiritty est un binaire TUI en Rust conçu pour combiner un agent d'assistance IA (orienté système, DevOps et terminal) et un terminal PTY interactif dans un écran partagé (*split-screen*).

---

## 📐 Standards de Code & Bonnes Pratiques Rust

1. **Rust Idiomatique & Moderne (Edition 2021+) :**
   - Utiliser `clippy` et `rustfmt` rigoureusement (`cargo clippy -- -D warnings`).
   - Préférer la composition et les `traits` bien délimités.
   - Éviter `unwrap()` ou `expect()` dans le code applicatif non critique ; propager les erreurs avec `thiserror` ou `anyhow`.

2. **Gestion Asynchrone (Tokio & Channels) :**
   - Ne **JAMAIS** bloquer la boucle de rendu TUI ou le thread principal.
   - Les tâches longues (appels LLM, I/O PTY, analyse de système) doivent s'exécuter dans des tâches `tokio::spawn` dédiées.
   - La communication entre les sous-systèmes (UI, PTY, LLM Agent) se fait par passage de messages typés via `tokio::sync::mpsc`.

3. **Performance & Empreinte Mémoire :**
   - Zéro lag de frappe sur le PTY : la réactivité du shell droit doit être indiscernable d'un terminal natif.
   - Limiter les allocations inutiles dans la boucle d'événement (`crossterm::event`).
   - Rendu ciblé avec `ratatui` en utilisant des widgets modulaires et réutilisables.

4. **Sécurité & Human-in-the-Loop :**
   - Aucune commande ne doit être envoyée ou injectée dans le PTY sans action/consentement explicite de l'utilisateur, sauf si un mode "YOLO / Auto-pilot" est expressément activé par configuration.
   - Classifier les propositions de commandes (Safe / Medium / Dangerous / Sudo).

5. **Internationalisation & Multilingue (i18n) :**
   - Toute chaîne visible dans l'UI (titres, modales, descriptions, boutons, statuts, messages d'erreur) et les system prompts de l'agent doivent passer par le module `src/i18n/`.
   - Détection automatique de la langue via `$LANG` / `$LC_ALL` avec possibilité de surcharge dans `config.toml` (`language = "fr" | "en"`).
   - Utiliser un catalogue typé (énumération de clés de traduction) pour garantir à la compilation qu'aucune traduction ne manque dans une langue supportée.

---

## 🏛️ Structure des Modules Recommandée

```
src/
├── main.rs                 # Point d'entrée, initialisation du terminal et de l'Event Loop
├── app.rs                  # État global de l'application (Focus, Mode, Données)
├── event.rs                # Gestionnaire d'événements centralisé (Clavier, PTY, LLM, Timers)
├── i18n/                   # Moteur d'internationalisation
│   ├── mod.rs              # Trait & structure I18n, détection de locale
│   ├── en.rs               # Dictionnaire Anglais
│   └── fr.rs               # Dictionnaire Français
├── ui/                     # Rendu graphique Ratatui
│   ├── mod.rs              # Layout global (Split gauche/droite, Header, Footer)
│   ├── chat_panel.rs       # Widget panneau d'assistance IA (Messages, Input, Preview commande)
│   ├── terminal_panel.rs   # Widget d'affichage du buffer PTY (vt100 screen buffer)
│   └── components/         # Modales, popups de validation, barres d'état
├── pty/                    # Gestion du Pseudo-Terminal
│   ├── mod.rs              # Abstraction PTY
│   ├── process.rs          # Cycle de vie du processus shell ($SHELL, spawn, resize)
│   └── vt.rs               # Parseur ANSI/VT100 et pont vers le buffer Ratatui
├── agent/                  # Cœur de l'Agent IA
│   ├── mod.rs              # Trait Agent & moteur de dialogue
│   ├── prompt.rs           # System prompts localisés, injection de contexte
│   ├── tools.rs            # Définition des outils (Run command, Read file, Get system info)
│   └── providers/          # Connecteurs LLM
│       ├── mod.rs          # Trait LLMProvider (streaming, function calling)
│       ├── ollama.rs       # Provider local Ollama
│       ├── gemini.rs       # Provider Google Gemini
│       ├── anthropic.rs    # Provider Anthropic Claude
│       └── openai.rs       # Provider OpenAI-compatible
├── system/                 # Inspection et contexte système
│   ├── mod.rs              # Détecteur d'environnement
│   ├── os_info.rs          # OS, Distro, Kernel, Arch
│   └── packages.rs         # Gestionnaires de paquets détectés (apt, pacman, brew, etc.)
└── config/                 # Gestion de la configuration
    └── mod.rs              # Chargement ~/.config/spiritty/config.toml
```

---

## 🧪 Stratégie de Test et Validation

1. **Tests Unitaires :** Tester les parseurs de prompts, la détection système, les règles de classification de commandes et la gestion de configuration.
2. **Tests d'Intégration :** Tester l'émulation VT100 et la synchronisation des channels `mpsc`.
3. **Validation Visuelle / TUI :** S'assurer que le redimensionnement de la fenêtre (`TerminalResize`) propage correctement la nouvelle taille au PTY (`pty.resize()`) sans glitch d'affichage.

---

## 🔄 Flux de Travail pour l'Agent IA

Lors de l'implémentation de nouvelles fonctionnalités :
1. Consulter [ARCHITECTURE.md](file:///home/xorne/Projets/Spiritty/ARCHITECTURE.md) pour respecter les invariants de conception.
2. Mettre à jour [ROADMAP.md](file:///home/xorne/Projets/Spiritty/ROADMAP.md) au fur et à mesure de l'avancement des tâches.
3. Toujours vérifier la compilation (`cargo check`) et l'absence d'erreurs de lint (`cargo clippy`).
4. **Gestion de Version & Commits :** Ne **JAMAIS** faire de `git commit` ou `git push` automatiquement de sa propre initiative. Les commits et pushs sont réservés à la **demande expresse** de l'utilisateur.
