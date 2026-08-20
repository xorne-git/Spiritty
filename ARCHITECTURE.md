# Architecture Technique de Spiritty

Ce document détaille les choix d'ingénierie et l'architecture interne de **Spiritty**.

---

## 1. Vue d'Ensemble & Boucle d'Événements (Event Loop)

Spiritty repose sur une architecture orientée événements asynchrone orchestrée par `tokio`. Le thread principal gère le rendu TUI et la capture des entrées utilisateur brutes (`crossterm::event`), tandis que des tâches d'arrière-plan gèrent les I/O du PTY et les requêtes de streaming vers les LLMs.

```
                    +--------------------------------+
                    |        Entrées Clavier         |
                    |   (crossterm::event::Event)    |
                    +---------------+----------------+
                                    |
                                    v
+-------------------+       +---------------+       +--------------------+
|  PTY Reader Task  | ----> |   mpsc Event  | <---- | LLM Streaming Task |
| (Sorties Shell)   |       |    Channel    |       |  (Tokens & Tools)  |
+-------------------+       +-------+-------+       +--------------------+
                                    |
                                    v
                        +-----------------------+
                        |   Event Loop (App)    |
                        | - Mise à jour d'état  |
                        | - Routage des inputs  |
                        +-----------+-----------+
                                    |
                                    v
                        +-----------------------+
                        |  Rendu TUI (Ratatui)  |
                        | - Panel Chat (Gauche) |
                        | - Panel Shell (Droit) |
                        +-----------------------+
```

---

## 2. Sous-Système PTY & Émulation VT100

Pour intégrer un terminal réel à l'intérieur d'un widget Ratatui sans perturber le shell parent :

1. **Création du PTY (`portable-pty`) :**
   - Un pseudo-terminal maître/esclave est alloué.
   - Le shell de l'utilisateur (défini par `$SHELL`, ex: `/bin/zsh`) est instancié sur l'esclave.
   - Les variables d'environnement (`TERM=xterm-256color`, `COLORTERM=truecolor`, etc.) sont injectées.

2. **Émulation d'Écran (`vt100`) :**
   - Le flux d'octets brut généré par le shell (contenant des codes ANSI d'échappement, déplacements de curseur, couleurs) est lu en asynchrone par un thread d'I/O.
   - Ces octets sont transmis à une instance de `vt100::Parser` qui maintient un écran virtuel 2D en mémoire.

3. **Rendu dans Ratatui :**
   - Lors de la passe de rendu `draw()`, le widget `TerminalPanel` lit les cellules de l'écran virtuel `vt100` et les convertit en cellules `ratatui::buffer::Buffer` (caractères, couleurs de premier plan/arrière-plan, attributs gras/souligné/inversé).
   - En cas de redimensionnement de l'interface (Split slider ou redimensionnement de la fenêtre), une notification `pty.resize(rows, cols)` est immédiatement envoyée au PTY esclave via `SIGWINCH`.

---

## 3. Système d'Agent IA & Injection de Contexte

### 3.1. Extracteur de Contexte Système (`system::Context`)
Avant d'envoyer un prompt au LLM, l'agent enrichit dynamiquement la requête avec les métadonnées de l'hôte :
- **OS / Distribution :** Détecté via `/etc/os-release` (ex: Ubuntu 24.04, Arch Linux, Fedora 40, macOS Sonoma).
- **Gestionnaires de paquets :** Présence de `apt`, `pacman`, `dnf`, `zypper`, `brew`, `nix`, `cargo`, `pip`, etc.
- **Environnement Shell :** Nom du shell, répertoire courant (`$PWD`), utilisateur actuel (`whoami`, root ou non).
- **Dernier contexte d'exécution :** Sorties récentes et codes d'erreur (`$?`) du panneau terminal.

### 3.2. Prompts & Rôles
L'agent utilise un `System Prompt` spécialisé lui intimant d'agir comme un expert en administration système et ligne de commande :
- Concision des réponses (adaptée au format terminal).
- Privilégier les commandes exactes et modernes (ex: `ss` plutôt que `netstat`, `ip` plutôt que `ifconfig`, `rg` plutôt que `grep`).
- Structuration des actions sous forme de blocs de commandes exécutables avec niveau de risque explicite.

---

## 4. Sécurité & Human-in-the-Loop (Validation des Commandes)

L'agent ne tape jamais directement dans le shell sans contrôle :

1. **Proposition d'action :** L'agent produit une commande avec une justification courte.
2. **Analyse de criticité (Heuristique de risque) :**
   - 🟢 **Faible (Read-only) :** `cat`, `ls`, `grep`, `systemctl status`, `df -h`
   - 🟡 **Moyen (Modification locale) :** `touch`, `mkdir`, `git commit`, `cargo build`
   - 🔴 **Élevé (Impact système / Sudo / Suppression) :** `sudo ...`, `rm -rf`, `dd`, `mkfs`, `iptables`, `systemctl restart`
3. **Contrôle utilisateur :**
   - `[Enter]` : Injecte la commande dans le PTY et l'exécute.
   - `[Tab]` : Copie la commande dans l'invite de commande du panneau droit pour modification avant exécution.
   - `[Esc / Backspace]` : Rejette la proposition et informe l'agent.

---

## 5. Abstraction Multi-Fournisseurs LLM

Spiritty implémente un trait unifié pour la communication avec les modèles :

```rust
#[async_trait]
pub trait LlmProvider: Send + Sync {
    async fn stream_completion(
        &self,
        messages: &[ChatMessage],
        context: &SystemContext,
        event_sender: mpsc::UnboundedSender<AgentEvent>,
    ) -> Result<()>;
}
```

### Providers Prévus :
1. **Ollama :** API locale HTTP (`http://localhost:11434/api/chat`), zéro dépendance externe, streaming direct, idéal en environnement déconnecté.
2. **Google Gemini :** API REST native (Interactions / GenerateContent) via `reqwest` pour une latence minimale et des fenêtres de contexte étendues.
3. **Anthropic Claude :** Support de Claude 3.5 Sonnet / Haiku pour le raisonnement système complexe.
4. **OpenAI / DeepSeek / Grok / LM Studio :** Format universel `v1/chat/completions`.

---

## 6. Moteur d'Internationalisation (i18n)

Spiritty prend en charge nativement le multilingue (Français, Anglais) au niveau de l'interface TUI et des instructions de l'agent :

1. **Détection & Configuration de la Langue :**
   - Ordre de priorité : Configuration utilisateur (`~/.config/spiritty/config.toml` -> `language = "fr" | "en"`) > Variable d'environnement (`$LANG`, `$LC_ALL`, `$LC_MESSAGES`) > Fallback Anglais (`en`).
2. **Catalogue de Traduction Typé à la Compilation :**
   - Un catalogue basé sur un `enum I18nKey` garantit à 100% lors du `cargo check` qu'aucune chaîne de traduction n'est manquante ou orpheline.
   - Accès ultra-rapide par correspondance de pointeurs statiques (`&'static str`), sans parsing runtime de fichiers JSON ou PO.
3. **Localisation des System Prompts :**
   - L'agent IA adapte sa langue de conversation et ses tournures d'explications selon la langue configurée tout en conservant les termes techniques et les commandes exactes.

---

## 7. Gestion des Erreurs et Robustesse

- Toutes les erreurs I/O (PTY mort, LLM timeout, fichier de configuration corrompu) sont transformées en événements typés `AppEvent::Error` ou affichées dans la barre de statut sans faire crasher l'application.
- Les panic hooks restaurent immédiatement le mode raw du terminal pour ne pas corrompre le shell de l'utilisateur.

---

## 8. Persistance des Sessions & Compactage de Contexte

Spiritty intègre un gestionnaire complet de sessions et d'optimisation de mémoire de contexte :

1. **Stockage Structuré JSON (`~/.config/spiritty/sessions/<id>.json`) :**
   - Persistance automatique de l'historique complet des messages (`User`, `Assistant`, `System`).
   - Sauvegarde de l'historique des prompts utilisateur (`prompt_history`) permettant de restaurer immédiatement la navigation avec `▲ / ▼`.
   - Métadonnées complètes : horodatage, total de tokens consommés, modèle et provider associés, titre déduit automatiquement de la première question.
2. **Compactage Automatique de Contexte (`Session::compact()`) :**
   - Lorsque l'historique dépasse 4 messages, les anciens tours de dialogue sont résumés sous forme de liste structurée (commandes `💻` exécutées, points clés) et condensés dans un bloc de synthèse.
   - Les 4 messages les plus récents sont conservés tels quels pour assurer la continuité naturelle de la conversation.
   - Le compactage s'exécute automatiquement lors de la fermeture de Spiritty (`save_current_session`), lors de la création d'une nouvelle session (`Ctrl + N`) et lors du chargement d'une autre session.
3. **Compatibilité stricte des Templates Jinja / LLM :**
   - Pour respecter l'invariant strict des modèles Hugging Face / Qwen / Llama 3 (`System message must be at the beginning`), les messages de contexte archivés sont transmis avec le rôle `user` dans les requêtes de streaming API, évitant tout crash HTTP 500.
