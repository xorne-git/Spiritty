# Spiritty 👻⚡

> **L'assistant IA pour terminal nouvelle génération, pensé pour les sysadmins, DevOps et power-users.**

Spiritty est une application TUI (Terminal User Interface) écrite en **Rust** qui combine dans un même écran scindé :
- **À gauche :** Un agent d'assistance IA contextuel, proactif et interactif.
- **À droite :** Votre terminal/shell par défaut (bash, zsh, fish) 100% interactif via PTY natif.

---

## 🎯 Pourquoi Spiritty ?

Les assistants CLI existants souffrent de deux écueils majeurs :
1. **Perte de contrôle :** Ils agissent en ligne de commande opaque sans shell interactif persistant.
2. **Focus exclusif sur le code :** Ils sont pensés pour éditer des repositories git, pas pour administrer un serveur, déboguer un service Linux, configurer un réseau ou installer des dépendances système.

**Spiritty comble ce vide** en offrant un copilote d'administration système avec validation humaine stricte (*human-in-the-loop*), capable de comprendre votre système d'exploitation, de diagnostiquer des erreurs et d'exécuter des actions directement dans votre environnement.

---

## 🏗️ Architecture & Layout

```
+-------------------------------------------------------------------------+
|                                SPIRITTY                                 |
+------------------------------------+------------------------------------+
|  🤖 AGENT IA (Panneau Gauche)      |  💻 SHELL INTERACTIF (Panneau Droit)
|                                    |                                    |
|  > "Configure un reverse proxy     |  $ caddy run --config ...          |
|     Caddy pour mon app sur :8080"  |  2026/08/19 15:00:00 [INFO] admin  |
|                                    |  2026/08/19 15:00:00 [ERROR] bind  |
|  [Agent] J'ai détecté Arch Linux.  |  address already in use :80        |
|  Le port 80 est déjà occupé.       |                                    |
|  Vérifions le processus actif :    |  $ sudo ss -tulpn | grep :80       |
|                                    |                                    |
|  Proposition de commande :         |                                    |
|  `sudo ss -tulpn | grep :80`       |                                    |
|                                    |                                    |
|  [Enter: Exécuter | Tab: Éditer]   |                                    |
+------------------------------------+------------------------------------+
| [Ctrl+Tab: Basculer Focus] [Ctrl+Q: Quitter] [Ctrl+N: Nouveau Chat]    |
+-------------------------------------------------------------------------+
```

---

## 🛠️ Stack Technique

- **Langage :** [Rust](https://www.rust-lang.org/) (Performance, sécurité mémoire, binaire autonome zéro-dépendance).
- **Interface TUI :** [`ratatui`](https://ratatui.rs/) & [`crossterm`](https://crates.io/crates/crossterm).
- **Gestion PTY :** [`portable-pty`](https://crates.io/crates/portable-pty).
- **Émulation de Terminal (VT100/ANSI) :** [`vt100`](https://crates.io/crates/vt100).
- **Runtime Asynchrone :** [`tokio`](https://tokio.rs/).
- **Connectivité LLM :** Multi-fournisseurs (Ollama local, Gemini, Claude/Anthropic, OpenAI, DeepSeek).

---

## 🚀 Fonctionnalités Clés

- [x] **Split-Screen Ergonomique :** Agent à gauche, Shell natif interactif (`$SHELL`) à droite avec redimensionnement interactif (souris ou `Alt+Left/Right`).
- [x] **Multi-Fournisseurs LLM :** Support complet pour LM Studio, Ollama local, Google Gemini, Anthropic Claude, OpenAI, DeepSeek et xAI (Grok) avec détection automatique de la taille de contexte.
- [x] **Gestionnaire de Sessions & Compactage :**
  - Sauvegarde et restauration complètes des sessions dans `~/.config/spiritty/sessions/`.
  - Modale interactive de sessions (`Ctrl + H`) et nouvelle session instantanée (`Ctrl + N`).
  - Restauration de l'historique des prompts utilisateur (`▲` / `▼`).
  - Compactage automatique intelligent de la mémoire à la fermeture et au changement de session.
- [x] **Validation Humaine (Human-in-the-Loop) & Cartes de Commandes :**
  - Détection automatique des commandes et badges de sécurité (🟢 Safe / 🟡 Sudo / 🔴 Risky).
  - Exécution directe au clavier avec `Alt + 1..9` ou `Enter`.
  - Analyse proactive et immédiate du résultat retourné par le shell dans l'agent.
- [x] **Interactions Souris & Raccourcis Clavier :**
  - Bascule de focus au clic (`🖱`), défilement à la molette (`🖱 Molette / PgUp/PgDn`).
  - Sélection de texte à la souris et copie automatique dans le presse-papier système (Wayland / X11).
  - Modale de configuration dynamique (`Ctrl + P`) et modale d'aide aux touches (`F1`).
- [x] **Conscience Système & Détection SSH Dynamique :**
  - Profiling multi-serveurs automatique (`hosts.json`) et bascule instantanée du prompt lors des connexions SSH.
  - Exécution 100% silencieuse sans pollution visuelle ni sentinelles dans le terminal.
  - Shell live et non-bloquant : tapez vos commandes librement pendant la génération du modèle.
- [x] **Éditeur de Prompt Multi-Lignes :**
  - `Shift + Enter`, `Alt + Enter`, `Ctrl + Enter` et `Ctrl + J` pour rédiger des prompts multi-lignes.
- [x] **Niveaux d'Approbation (Auto-Approve) :**
  - Cycle rapide avec `F3` : 🟢 Safe / 🟡 Sudo / 🔴 YOLO / ⚫ Off.
- [x] **Internationalisation (i18n) :** Français et Anglais avec détection automatique via `$LANG`.

---

## ⌨️ Raccourcis Clavier Principaux

| Raccourci | Action |
| :--- | :--- |
| `Entrée` | Envoyer le prompt (Chat) ou valider une commande (Terminal) |
| `Shift + Entrée` / `Ctrl + J` | Insérer un retour à la ligne dans l'éditeur de prompt |
| `Ctrl + Espace` ou `Shift + Tab` | Basculer le focus (Chat ↔ Terminal) |
| `Alt + 1` .. `Alt + 9` | Exécuter directement la proposition de commande N |
| `F3` | Changer le mode d'approbation automatique (Safe / Sudo / YOLO / Off) |
| `Ctrl + H` | Ouvrir le gestionnaire de sessions |
| `Ctrl + N` | Créer une nouvelle session vierge |
| `Ctrl + P` | Ouvrir la configuration des modèles / API keys |
| `F1` | Afficher la modale d'aide des raccourcis |
| `Alt + ←` / `Alt + →` | Déplacer la séparation d'écran |
| `Ctrl + Q` | Sauvegarder et quitter Spiritty |

---

## 📂 Documentation du Projet

- 📐 **[ARCHITECTURE.md](file:///home/xorne/Projets/Spiritty/ARCHITECTURE.md)** : Spécifications techniques et conception des sous-systèmes.
- 🗺️ **[ROADMAP.md](file:///home/xorne/Projets/Spiritty/ROADMAP.md)** : Étapes de développement et jalons des versions.
- 🤖 **[AGENTS.md](file:///home/xorne/Projets/Spiritty/AGENTS.md)** : Directives de développement et règles pour les assistants IA contribuant au projet.

---

## 📄 Licence

MIT ou Apache 2.0 (au choix).
