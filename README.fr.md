<div align="center">
  <img src="assets/logo.png" alt="Logo Spiritty" width="220">
  <h1>Spiritty 🧞⚡</h1>
  <p><strong>L'assistant IA pour terminal nouvelle génération, pensé pour les sysadmins, DevOps et power-users.</strong></p>
  <p><a href="README.md">English</a> | <strong>Français</strong></p>
</div>

Spiritty est une application TUI (Terminal User Interface) écrite en **Rust** qui combine dans un même écran scindé :
- **À gauche :** Un agent d'assistance IA contextuel, proactif et interactif.
- **À droite :** Votre terminal/shell par défaut (bash, zsh, fish) 100% interactif via PTY natif.

---

## ⚡ Installation Rapide

Installez Spiritty en une seule commande (Linux & macOS) :

```bash
curl -fsSL https://raw.githubusercontent.com/xorne-git/Spiritty/main/install.sh | bash
```

*Ou compilez depuis les sources :*
```bash
git clone https://github.com/xorne-git/Spiritty.git
cd Spiritty
cargo build --release
sudo cp target/release/spiritty /usr/local/bin/
```

---

## 🎯 Pourquoi Spiritty ?

J'administre des serveurs Linux depuis bientôt 25 ans. 😊

Les outils de terminal assistés par IA existants ne me convenaient pas : trop lourds, interfaces graphiques encombrantes, usines à gaz qui tentent de tout faire mais peinent à accomplir simplement les tâches réelles d'administration système du quotidien. En CLI pure, il n'existait quasiment rien d'adapté.

J'ai donc décidé de me mettre à Rust et de développer l'outil dont j'avais réellement besoin : se connecter en SSH à un VPS, auditer et optimiser Apache, PHP-FPM ou MySQL avec l'aide d'un modèle d'IA, basculer sur un autre serveur et dire à l'agent « *fais pareil ici* », et enchaîner ainsi sans jamais quitter son environnement de terminal conçu précisément pour cette mission.

**Spiritty** comble ce vide en offrant un copilote d'administration système avec validation humaine stricte (*human-in-the-loop*), capable de comprendre votre système d'exploitation, de diagnostiquer des pannes et d'exécuter des actions de manière sécurisée et transparente.

> *C'est une première version bêta. Si cet outil peut vous être utile, tant mieux ! Tous les retours et remarques constructives sont les bienvenus.*

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
|  [Alt+N : Exécuter la proposition] |                                    |
+------------------------------------+------------------------------------+
| [Ctrl+Espace: Focus] [Ctrl+Q: Quitter] [Ctrl+N: Nouveau Chat]            |
+-------------------------------------------------------------------------+
```

---

## 🛠️ Stack Technique

- **Langage :** [Rust](https://www.rust-lang.org/) (Performance, sécurité mémoire, binaire autonome zéro-dépendance).
- **Interface TUI :** [`ratatui`](https://ratatui.rs/) & [`crossterm`](https://crates.io/crates/crossterm).
- **Gestion PTY :** [`portable-pty`](https://crates.io/crates/portable-pty).
- **Émulation de Terminal (VT100/ANSI) :** [`vt100`](https://crates.io/crates/vt100).
- **Runtime Asynchrone :** [`tokio`](https://tokio.rs/).
- **Connectivité LLM :** Multi-fournisseurs (Ollama local, LM Studio, Gemini, Claude/Anthropic, OpenAI, DeepSeek, Grok, Z.ai GLM).

---

## 🚀 Fonctionnalités Clés

- [x] **Split-Screen Ergonomique :** Agent à gauche, Shell natif interactif (`$SHELL`) à droite avec redimensionnement interactif (souris ou `Alt+Left/Right`).
- [x] **Multi-Fournisseurs LLM :** Support complet pour LM Studio, Ollama local, Google Gemini, Anthropic Claude, OpenAI, DeepSeek, xAI (Grok) et Z.ai (GLM) avec détection automatique de la taille de contexte.
- [x] **Gestionnaire de Sessions & Compactage :**
  - Sauvegarde et restauration complètes des sessions dans `~/.config/spiritty/sessions/`.
  - Modale interactive de sessions (`Ctrl + H`) et nouvelle session instantanée (`Ctrl + N`).
  - Restauration de l'historique des prompts utilisateur (`▲` / `▼`).
  - Historique **complet** persisté et restauré — le défilement remonte l'intégralité des échanges, même après rechargement (fini le plafond à 9 messages dans la liste des sessions).
  - Compactage intelligent appliqué **uniquement au contexte LLM** : les tours anciens roulent dans un **résumé structuré** (message System) tandis que les 8 plus récents sont conservés verbatim — le coût en tokens reste borné sur les sessions longues.
- [x] **Validation Humaine (Human-in-the-Loop) & Cartes de Commandes :**
  - Détection automatique des commandes et badges de sécurité (🟢 Safe / 🟡 Standard / 🟣 Sudo / 🔴 Risky).
  - Exécution directe au clavier avec `Alt + 1..9`, et `F10` pour autoriser une commande en attente.
  - Analyse proactive et immédiate du résultat retourné par le shell dans l'agent.
- [x] **Interactions Souris & Raccourcis Clavier :**
  - Bascule de focus au clic (`🖱`), défilement à la molette (`🖱 Molette / PgUp/PgDn`).
  - Sélection de texte à la souris et copie automatique dans le presse-papier système (Wayland / X11).
  - Modale de configuration dynamique (`Ctrl + P`) et modale d'aide aux touches (`F1`).
- [x] **Conscience Système & Détection SSH Dynamique :**
  - Profiling multi-serveurs automatique (`hosts.json`) et bascule instantanée du prompt lors des connexions SSH.
  - Exécution 100% silencieuse sans pollution visuelle ni sentinelles dans le terminal.
  - Shell live et non-bloquant : tapez vos commandes librement pendant la génération du modèle.
  - Modale de reconnexion SSH en un appui (`⏎ Se reconnecter`) au rechargement d'une session qui était distante alors que le PTY est local, avec hint persistant `· 🔗 SSH (reprise)` dans le titre.
- [x] **Éditeur de Prompt Multi-Lignes :**
  - `Shift + Enter`, `Alt + Enter`, `Ctrl + Enter` et `Ctrl + J` pour rédiger des prompts multi-lignes.
- [x] **Niveaux d'Approbation (Auto-Approve) :**
  - Cycle rapide avec `F3` : 🟢 Safe / 🟡 Sudo / 🔴 YOLO / ⚫ Off.
- [x] **Internationalisation (i18n) :** Français et Anglais avec détection automatique via `$LANG`.

---

## 🚀 Options de Lancement en Ligne de Commande (CLI)

Spiritty propose des arguments en ligne de commande pour s'intégrer directement dans vos flux de travail :

```bash
# Reprendre la dernière session active
spiritty -c

# Reprendre une session spécifique par son ID ou préfixe de titre
spiritty -s sess_20260824_0001
spiritty -s nginx

# Poser une question directement au lancement
spiritty "Vérifie l'utilisation de la mémoire et les logs d'erreur"

# Se connecter directement à un serveur SSH
spiritty --ssh root@vps-web.prod:22

# Remplacer temporairement le modèle ou fournisseur d'IA
spiritty --model qwen2.5-coder:7b --yolo

# Lister toutes les sessions enregistrées
spiritty --list-sessions

# Afficher l'aide complète des commandes CLI
spiritty --help
```

---

## ⌨️ Raccourcis Clavier Principaux

| Raccourci | Action |
| :--- | :--- |
| `Entrée` | Envoyer le prompt (Chat) ou valider une commande (Terminal) |
| `Shift + Entrée` / `Ctrl + J` | Insérer un retour à la ligne dans l'éditeur de prompt |
| `Ctrl + Espace` ou `Shift + Tab` | Basculer le focus (Chat ↔ Terminal) |
| `Alt + 1` .. `Alt + 9` | Exécuter directement la proposition de commande N |
| `F10` | Autoriser la commande en attente (validation en un appui) |
| `F6` | Basculer le focus (alias de `Ctrl + Espace`) |
| `F3` | Changer le mode d'approbation automatique (Safe / Sudo / YOLO / Off) |
| `Ctrl + B` | Gestionnaire de serveurs SSH & favoris (Quick-Connect) |
| `Ctrl + E` | Exporter la session active en rapport Markdown |
| `Ctrl + F` | Rechercher dans l'historique du chat en temps réel |
| `Alt + D` | Diagnostic et remédiation proactive d'une erreur |
| `Ctrl + H` | Ouvrir le gestionnaire de sessions |
| `Ctrl + N` | Créer une nouvelle session vierge |
| `Ctrl + P` | Ouvrir la configuration des modèles / API keys |
| `F1` | Afficher la modale d'aide des raccourcis |
| `Alt + ←` / `Alt + →` | Déplacer la séparation d'écran |
| `Ctrl + Q` | Sauvegarder et quitter Spiritty |

---

## 📂 Documentation du Projet

- 📐 **[ARCHITECTURE.md](ARCHITECTURE.md)** : Spécifications techniques et conception des sous-systèmes.
- 🗺️ **[ROADMAP.md](ROADMAP.md)** : Étapes de développement et jalons des versions.
- 🤖 **[AGENTS.md](AGENTS.md)** : Directives de développement et règles pour les assistants IA contribuant au projet.
- 📜 **[CHANGELOG.md](CHANGELOG.md)** : Historique détaillé des modifications entre chaque release.

---

## 📄 Licence

MIT ou Apache 2.0 (au choix).
