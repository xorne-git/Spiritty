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

---

## v0.7.3 — 2026-09-10

### Ajouté

- **Nouveau logo officiel et icône de bureau multi-résolutions (Génie céleste Spiritty)** :
  - Intégration du nouveau logo officiel haute résolution dans [`assets/logo.png`](file:///home/xorne/Projets/Spiritty/assets/logo.png) et [`assets/icons/spiritty.png`](file:///home/xorne/Projets/Spiritty/assets/icons/spiritty.png) (génie céleste bleu et or aux volutes stellaires en forme de « S »).
  - Déploiement de l'icône de bureau dans toutes les tailles standard FreeDesktop (`16x16`, `32x32`, `48x48`, `64x64`, `128x128`, `256x256`, `512x512` et SVG vectoriel `scalable`).
  - Embarquement direct dans [`src/brand.rs`](file:///home/xorne/Projets/Spiritty/src/brand.rs) (`ICON_PNG` et `ICON_SVG`).
  - Intégration visuelle dans `README.md`, `README.fr.md`, le script d'installation `install.sh` et le pipeline CI de release.
- **Refonte visuelle et conteneur encadré des cartes de commandes (fidèle au design du site web)** :
  - Refonte complète de [`render_command_card`](file:///home/xorne/Projets/Spiritty/src/ui/chat_panel.rs) et [`compose_approval_card`](file:///home/xorne/Projets/Spiritty/src/ui/chat_panel.rs) :
    - Conteneur externe englobant aux coins arrondis (`╭─...─╮`, `│...│`, `╰─...─╯`).
    - En-tête interne : éclair `⚡` et `COMMANDE #N` ambrés avec badge de risque sobre encadré (`[ SAFE ]`, `[ RISQUÉ ]`).
    - Boîte intérieure encadrée dédiée au code avec texte en cyan lumineux (`Color::LightCyan`).
    - Pied de carte épuré : libellé de statut (`"Validation requise avant exécution"`) à gauche et bouton touche `[ Alt + N ]` ambré à droite (sans suffixe verbeux `"Exécuter"`).
    - Harmonisation de la carte d'approbation d'outils avec touches encadrées `[ F10 ]` et `[ Esc ]`.

### Corrigé

- **Sélection et copie souris de la dernière ligne du prompt multiligne** :
  - Correction dans [`src/ui/mod.rs`](file:///home/xorne/Projets/Spiritty/src/ui/mod.rs) du calcul du rectangle de sélection (`inner.height = panel_area.height.saturating_sub(1)` au lieu de `saturating_sub(2)`).
  - Auparavant, la double soustraction de bordure (`height - 2` puis `clamp(..., bottom - 1)`) tronquait la sélection à `panel_area.bottom() - 2`, empêchant la sélection et la copie de la 3ème ligne d'un prompt multiligne (ou de toute ligne inférieure du prompt et du terminal).
  - La sélection à la souris peut désormais englober sans coupure l'intégralité du prompt multiligne jusqu'à la dernière ligne (`panel_area.bottom() - 1`).

---

## v0.7.2 — 2026-09-10

### Corrigé

- **Isolation des sessions de tests et élimination de la pollution de l'historique de sessions** :
  - Correction d'un effet de bord où l'exécution de la suite de tests (`cargo test`) enregistrait des sessions temporaires de test directement dans `~/.config/spiritty/sessions/` sans les supprimer.
  - La commande `spiritty -c` reprenait alors ces sessions de test orphelines (ex. `Session #141311` avec la commande dummy `sed -n '1,10p' Cargo.toml`) au lieu de la dernière vraie conversation utilisateur, et polluait le gestionnaire de sessions.
  - Ajout du support de la variable `SPIRITTY_SESSIONS_DIR` dans [`src/session/storage.rs`](file:///home/xorne/Projets/Spiritty/src/session/storage.rs) pour isoler les tests, suppression systématique des artefacts de tests, et purge des sessions tests orphelines.
- **Robustesse du découpage des blocs de code Markdown et élimination des fausses propositions de commandes** :
  - Correction d'un bogue subtil dans [`src/app.rs`](file:///home/xorne/Projets/Spiritty/src/app.rs) et [`src/ui/chat_panel.rs`](file:///home/xorne/Projets/Spiritty/src/ui/chat_panel.rs) où la présence de triple backticks dans une chaîne littérale à l'intérieur d'un bloc de code (ex. `remaining.find("```")`) ou de backticks inline dans le texte conversationnel tronquait prématurément le bloc et interprétait des mots ordinaires du texte explicatif (ex. le mot « Pour ») comme des propositions de commandes shell exécutables.
  - Implémentation des analyseurs syntaxiques rigoureux `find_opening_code_fence` et `find_closing_code_fence` vérifiant qu'une fence d'ouverture ou de fermeture commence en début de ligne (avec espaces optionnels) et possède une balise info valide, ignorant les backticks inline ou contenus dans des chaînes littérales.
  - Amélioration de `repair_prematurely_closed_code_blocks` : lorsqu'un bloc vide prématurément fermé est détecté, si le texte suivant contient déjà des fences valides, la fence vide est supprimée sans ré-emballer les explications textuelles suivantes dans un faux bloc bash.
  - Filtrage renforcé dans `is_clean_command_line` rejetant les mots de liaison conversationnels isolés (« pour », « suite », « attention », « voici », « cela »).
- **Auto-approbation des propositions de commandes selon le niveau configuré (Safe / Sudo / Yolo)** :
  - Correction d'une incohérence où les commandes shell proposées par l'assistant sous forme de bloc Markdown interactif (`⚡ COMMANDE #1`) imposaient systématiquement une validation manuelle (`Alt + 1`), même lorsque le mode d'approbation actif (`F3 Safe` ou `F3 Sudo`) autorisait explicitement le niveau de risque de la commande (ex. commandes en lecture seule `Safe` comme `sed`, `cat`, `grep`, `df` ou commandes administratives `Sudo`).
  - Évaluation automatique dans [`src/app.rs`](file:///home/xorne/Projets/Spiritty/src/app.rs) (`on_agent_done`) des propositions uniques éligibles via `should_auto_approve_command(&cmd, level)` avec injection et exécution directe dans le terminal PTY.
  - Préservation stricte du contrôle utilisateur : les commandes destructrices (`Risky`) exigent toujours une validation manuelle (sauf en mode `Yolo`), les propositions alternatives multiples attendent le choix de l'utilisateur, et un garde-fou borne les enchaînements automatiques à 10 exécutions consécutives maximum avec notification toast i18n (`AutoApproveMaxConsecutiveReached`).
- **Restauration du prompt système pour les fournisseurs OpenAI-compatibles (DeepSeek, Grok, GLM, LM Studio)** :
  - Correction d'un bogue critique de masquage de variable (`let mut api_messages = Vec::new()`) dans [`src/agent/providers/openai.rs`](file:///home/xorne/Projets/Spiritty/src/agent/providers/openai.rs) qui écrasait et réinitialisait la liste des messages à vide juste après l'insertion du message système.
  - Les modèles DeepSeek reçoivent à nouveau le prompt système complet de Spiritty (identifiant le rôle, le terminal split-screen, les outils `tool:run_command`, `tool:web_search`, `tool:read_file`, etc.) et peuvent naviguer sur le web et inspecter le système au lieu de refuser en affirmant qu'ils n'ont pas d'accès Internet.
  - Extraction de la fonction pure `build_api_messages` couverte par une suite de tests unitaires dédiés.
- **Détection de la fenêtre de contexte pour toute la gamme DeepSeek (131k tokens)** :
  - Correction de la détection de la fenêtre de contexte maximale dans [`src/app.rs`](file:///home/xorne/Projets/Spiritty/src/app.rs) : le test ciblait restrictivement `model.contains("deepseek-v4")`, provoquant la dégradation de `deepseek-flash`, `deepseek-chat` et `deepseek-reasoner` vers la fenêtre locale par défaut de 8,2k tokens.
  - Élargissement à `model.contains("deepseek")` garantissant la fenêtre complète de 131 072 tokens (131k) pour tous les modèles de la famille DeepSeek.
- **Durcissement et sécurisation du parsing des blocs d'outils (`tool:run_command`, `web_search`, `read/write/edit_file`)** :
  - Correction d'un comportement critique dans [`src/agent/tools.rs`](file:///home/xorne/Projets/Spiritty/src/agent/tools.rs) où l'absence de délimiteur fermant ``` ou la mention inline d'un nom d'outil dans une phrase explicative provoquait la capture de l'intégralité du reste de la réponse Markdown comme commande shell exécutée en direct dans le PTY.
  - Exigence stricte d'un début de ligne (avec indentation optionnelle), d'une ligne d'en-tête propre terminée par un saut de ligne, et d'un délimiteur fermant obligatoire ``` (ou `</tool:...>`).
  - Filtrage des faux positifs : rejet systématique des commandes placeholders (`...`, `<command>`, `<unit>`, `cmd`, `commande`) et des blocs tronqués à mi-parcours.
  - Ajustement dans [`src/ui/chat_panel.rs`](file:///home/xorne/Projets/Spiritty/src/ui/chat_panel.rs) et [`src/app.rs`](file:///home/xorne/Projets/Spiritty/src/app.rs) pour préserver le texte conversationnel mentionnant des outils sans le tronquer arbitrairement.

### Ajouté

- **Support du modèle DeepSeek V4.1 Flash (`deepseek-flash`) et alignement tarifaire (10 sept. 2026)** :
  - Alignement sur l'identifiant réel de l'API DeepSeek : bien que l'annonce titre « V4.1 Flash », la passerelle officielle `api.deepseek.com` n'accepte que `deepseek-flash`, `deepseek-v4-flash` et `deepseek-v4-pro` (rejetant `deepseek-v4.1-flash` en HTTP 400).
  - Définition de `deepseek-flash` comme modèle officiel par défaut dans [`src/config/mod.rs`](file:///home/xorne/Projets/Spiritty/src/config/mod.rs).
  - Mappage automatique et systématique de toute sélection ou saisie d'un modèle `v4*` (`deepseek-v4`, `deepseek-v4-flash`, `deepseek-v4-pro`, `deepseek-v4.1-flash`) vers `deepseek-flash` au chargement de la configuration, dans la modale de configuration (F2/Ctrl+P) et sur le réseau dans [`src/agent/providers/openai.rs`](file:///home/xorne/Projets/Spiritty/src/agent/providers/openai.rs).
  - Mise à jour des tarifs officiels dans [`assets/pricing.json`](file:///home/xorne/Projets/Spiritty/assets/pricing.json) et [`src/pricing/mod.rs`](file:///home/xorne/Projets/Spiritty/src/pricing/mod.rs) ($0.30 / 1M input cache-miss, $1.20 / 1M output au plein tarif peak, avec réduction dynamique à $0.15 / $0.60 en heures creuses off-peak et week-ends).

---

## v0.7.1 — 2026-09-09

### Ajouté

- **Niveau de réflexion / raisonnement configurable par provider (`ReasoningEffort`)** :
  - **Sélecteur interactif dans la modale de configuration (`F2`)** : nouveau champ `5. Réflexion IA ❯ [←] Badge [→]` permettant d'ajuster le niveau de réflexion du modèle à la volée parmi 5 crans : `Default` (défaut du modèle), `Off` (désactivé pour vitesse maximale), `Low` (faible, budget ~1k tokens), `Medium` (moyen, budget ~4k tokens) et `High` (élevé, budget ~16k tokens), avec couleurs dynamiques et descriptions localisées (FR/EN).
  - **Intégration API multi-providers** :
    - *Google Gemini* : transmission de `generationConfig.thinkingConfig.thinkingBudget` (0 pour désactiver, 1024, 4096, 16384 ou omis par défaut).
    - *OpenAI / Compatible* : transmission du paramètre standard `reasoning_effort` (`low`, `medium`, `high`) dans les requêtes de complétion.
    - *Anthropic Claude* : activation de `thinking: { type: "enabled", budget_tokens: ... }` avec calcul automatique du plafond `max_tokens` (jusqu'à 20480 tokens) et repliement transparent des blocs `thinking_delta` dans les balises de réflexion `<think>...</think>`.
  - **Persistance par provider** : enregistrement propre dans `~/.config/spiritty/config.toml` sous chaque provider (`reasoning_effort = "..."`), omis automatiquement lorsque réglé sur `default`.
  - **Indicateur visuel dans la barre d'état** : affichage en temps réel du niveau de réflexion directement à droite du nom du modèle (` 🧠 Auto `, ` 🧠 Off `, ` 🧠 Low `, ` 🧠 Med `, ` 🧠 High `) avec code couleur dédié et adaptation responsive selon la largeur du terminal.

- **Actualisation dynamique des modèles et tarifs LLM en direct (`R` / `Ctrl+R` / `F5` dans la modale Configuration `F2`)** :
  - **Découverte dynamique multi-providers** : interrogation asynchrone non-bloquante des API officielles de chaque provider (Google Gemini via `/v1beta/models`, Anthropic via `/v1/models`, Ollama via `/api/tags`, providers OpenAI-compatibles via `/v1/models`) avec prise en compte instantanée des clés API et URLs personnalisées saisies dans la modale.
  - **Mise à jour et persistance automatique** : enregistrement immédiat des nouveaux modèles découverts dans la configuration de l'utilisateur (`~/.config/spiritty/config.toml`), mise à jour du sélecteur interactif de modèles et déclenchement simultané de la mise à jour des grilles tarifaires de tokens.
  - **Retour d'état visuel et i18n complète** : badge interactif `[ R ] Actualiser modèles` dans le pied de la modale avec messages de statut dynamiques (interrogation en cours, succès avec nombre de modèles découverts, ou cause d'échec explicite en cas de clé API manquante ou d'erreur réseau) en français et anglais.
  - **Affichage responsive et élargi du pied de modale** : élargissement de la largeur de la modale (`clamp(88, 130)` colonnes) et agencement réactif (mono-ligne aéré sur écran large, ou bi-lignes équilibré sur écran compact) garantissant la visibilité intégrale de l'ensemble des boutons et raccourcis (`Tab / ↑↓ Naviguer`, `Ctrl+S Enregistrer`, `R Actualiser modèles`, `U Tarifs en ligne`, `Échap Fermer`) sans aucun débordement ni troncature.
- **Support du modèle `gemini-3.8-flash` pour Google Gemini** :
  - Ajout de `gemini-3.8-flash` comme modèle par défaut et en tête de liste des modèles recommandés pour Google Gemini.
  - Intégration de la grille tarifaire (0.75 $ entrée / 3.75 $ sortie par million de tokens) dans le registre dynamique et le fallback embarqué (`assets/pricing.json`).
- **Robustesse des applications interactives ncurses & plein écran (`vim`, `nano`, `htop`, `fzf`, `lazygit`, `less`)** :
  - **SGR Mouse Reporting natif** : détection dynamique du protocole souris xterm/SGR (`\x1b[?1000h` / `\x1b[?1006h`) et retransmission instantanée des clics, relâchements, glissés et molettes vers le processus PTY (`htop`, `vim` avec `:set mouse=a`, `fzf`). Le maintien de la touche `Shift` permet de contourner la souris applicative pour sélectionner et copier du texte localement.
  - **Défilement molette intelligent dans l'alternate screen** : lorsqu'une application plein écran s'exécute sans protocole souris (`less`, `man`, `vim`), la molette de la souris émet automatiquement des touches fléchées vers le PTY au lieu de défiler un historique de scrollback vide.
  - **Encodage étendu xterm des touches de navigation avec modificateurs** : support complet de `Ctrl+Flèches` (saut de mot dans readline/zsh/nano), `Shift+Flèches`, `Alt+Flèches`, `Ctrl+Home`/`End`, `Ctrl+Delete`, et touches de fonction `F1..F12` avec modificateurs.
  - **Non-interception de `PageUp` / `PageDown` et `Ctrl+V` en alternate screen** : les touches `PageUp` et `PageDown` parviennent désormais directement à l'application active. `Ctrl+V` (sans Shift) est transmis à `vim` pour déclencher la sélection de bloc visuel (`^V`), tandis que `Ctrl+Shift+V` reste dédié au collage.
  - **Support natif du Bracketed Paste (`\x1b[?2004h`)** : encadrement automatique du texte collé par les marqueurs VT (`\x1b[200~` ... `\x1b[201~`) évitant les effets d'escalier d'indentation dans `vim`/`nano` et l'exécution prématurée de commandes multilignes.
- **Renommage d'onglets et persistance multi-onglets dans les sessions (`Alt+R`)** :
  - **Modale de renommage dédiée (`Alt+R`)** : permet d'attribuer un libellé métier clair à chaque onglet (ex: `bdd-prod`, `logs-nginx`) avec i18n complète (FR/EN) ou de revenir au titre dynamique par défaut avec une saisie vide. Raccourci documenté dans la modale d'aide (`F1` / `?`).
  - **Persistance des titres personnalisés et contextes SSH** : les onglets et leurs libellés sont sauvegardés de manière transparente au sein du fichier de session (`Session.tabs`) et fidèlement réappliqués lors du rechargement d'une session.

### Modifié

- **Affichage ultra-compact des sessions dans la barre d'état et espacement garanti** :
  - Remplacement de l'ancien message verbeux de restauration de session (qui contenait le titre complet, le nombre de messages, le statut SSH et le rappel d'approbation `⚡ Sudo` redondant) par une étiquette ultra-courte : `📂 Session #151237` (~17 caractères au total).
  - Élimination des collisions de texte dans le pied de page : réservation d'un espacement minimal garanti d'au moins 2 espaces entre les métriques de gauche et les raccourcis de droite (`build_right_shortcuts`), évitant tout chevauchement ou accolement de texte (`SudoApprobation`).
- **Architecture : unification et approfondissement des modales TUI ([`src/ui/components/`](file:///home/xorne/Projets/Spiritty/src/ui/components/))** :
  - Centralisation de la gestion des frappes (`handle_key`), du collage presse-papier (`handle_paste`) et du rendu visuel (`render`) de l'ensemble des 7 modales (`Help`, `Config`, `Sessions`, `Bookmarks`, `Export`, `Mcp`, `RenameTab`, `SshReconnect`) au sein de l'enum unifiée `ModalState` et de la machine à états de transition `ModalOutcome`.
  - Élimination de plus de 280 lignes de logique dispersée entre `src/app.rs` et `src/ui/mod.rs`, réduisant les blocs ad-hoc à de simples délégations composables et testables de façon isolée.
- **Architecture : création du superviseur système et hôtes SSH ([`src/system/supervisor.rs`](file:///home/xorne/Projets/Spiritty/src/system/supervisor.rs))** :
  - Encapsulation complète de la surveillance `/proc` des processus en avant-plan (`SystemSupervisor`), de la détection de session SSH/Docker/locale, de la synchronisation de répertoire courant et branche Git, ainsi que de l'exécution en arrière-plan des sondes d'inspection de distribution (`ssh -o BatchMode=yes ...`).
  - Découplage des onglets via le trait `InspectableTab` permettant de tester la détection de processus, l'état multi-onglets et le profilage système sans dépendance sur la boucle TUI ni le PTY réel. Déduplication active des sondes SSH en vol pour éviter toute saturation réseau.
- **Architecture : extraction et approfondissement du sous-système `ToolCapture` ([`src/pty/capture.rs`](file:///home/xorne/Projets/Spiritty/src/pty/capture.rs))**
  — allègement substantiel de `src/app.rs` (-690 lignes) par l'encapsulation complète de la capture d'outils, du décodage UTF-8 incrémental avec carry buffer, du balayage de sentinelles (`OSC 777`), du plafond d'overflow 1 Mio, de la détection des invites interactives (`InteractionKind`) et de l'annulation propre `SIGINT` au sein d'une machine à états pure `ToolCaptureSession` 100% testable en mode headless.

- **Élimination des blocages sur les pagers interactifs (`systemctl`, `journalctl`, `git`, `less`)** :
  - **Injection automatique de `--no-pager` (`ensure_non_interactive_command`)** : détection et injection systématique de l'option `--no-pager` lors de l'exécution de commandes système (`systemctl`, `journalctl`, `git log/diff/show/branch`) aussi bien en local qu'en session SSH ou sous `sudo`. Évite que des commandes de statut ou d'inspection ne lancent `less` en arrière-plan et ne bloquent le terminal sur une invite `lines ... (END)`.
  - **Détection et auto-acquittement des pagers (`InteractionKind::Pager`)** : extension de la détection d'invites interactives dans `src/pty/capture.rs` (`lines ... (END)`, `--More--`, etc.) et envoi automatique immédiat du caractère `q` au PTY afin de libérer le terminal sans requérir d'intervention manuelle de l'utilisateur.
  - **Filtrage des résidus de statut dans les sorties PTY** : suppression des lignes de statut de pager (`lines 1-25/25 (END)`) dans `clean_pty_output` pour ne pas polluer le contexte renvoyé à l'agent IA.
  - **Règle explicite dans le prompt système** : instruction formelle à l'agent IA d'éviter les commandes interactives et de toujours adjoindre `--no-pager` ou de rediriger vers `cat`/`head`.
- **Affichage de la réflexion en streaming pour Google Gemini (`includeThoughts: true`)** :
  - Transmission du paramètre `includeThoughts: true` au sein de `thinkingConfig` dans les requêtes vers l'API Google Gemini (lorsque `ReasoningEffort` est configuré sur `Low`, `Medium`, `High` ou `Default`). Sans ce drapeau explicite, l'API Gemini omettait les fragments de réflexion (`thought: true`) du flux SSE, empêchant l'affichage de la ligne animée `💭 Réflexion · ...` dans la fenêtre de chat.
  - Clôture propre du bloc de réflexion via `bracket.finish()` en fin de flux lorsqu'une réponse se termine par une phase de raisonnement.
- **Auto-approbation des commandes de diagnostic multi-lignes et enchaînées (`Safe` / `Sudo`)** :
  - **Découpage intelligent des chaînes de commandes (`split_chained_commands`)** : prise en charge complète des retours à la ligne (`\n`), des opérateurs logiques `&&`, `||`, des points-virgules `;` ainsi que des parenthèses de sous-shell `(...)` couramment générées par les modèles LLM lors de diagnostics complexes.
  - **Enrichissement du catalogue d'inspection sûre (`safe_prefixes`)** : classification automatique comme `Safe` des binaires et options de diagnostic usuels (`nproc`, `free`, `php -v/-m/-i`, `apache2 -v`, `apachectl -M/-S/-v`, `httpd`, `mysql --version`, `mariadb --version`, `dpkg -l/-s/--list`, `awk`, `sed` en lecture seule sans `-i`). Les audits système ne sont plus faussement dégradés en commandes soumises à validation manuelle.
- **Sécurisation par défaut de l'auto-approbation à `Safe`** :
  - Remplacement du comportement hérité où `auto_approve = true`, `all` ou `auto` activait le mode `Yolo`. La valeur booléenne `true` ou la chaîne `auto` bascule désormais strictement en `Safe` (commandes en lecture seule auto-approuvées, modifications système soumises à confirmation). Seule la valeur explicite `yolo` active le mode YOLO.
  - La reprise d'une session contenant un niveau YOLO n'écrase plus le fichier de configuration global `config.toml`, protégeant le réglage utilisateur par défaut entre les sessions.
- **Persistance et mémorisation du dernier modèle sélectionné** :
  - L'actualisation des modèles (`R`) synchronise immédiatement le modèle actuellement saisi ou sélectionné dans la configuration avant écriture sur disque, évitant toute régression vers un modèle antérieur.
  - La navigation entre différents fournisseurs (`←`/`→`) au sein de la modale `F2` conserve en mémoire tampon les modèles et paramètres choisis pour chaque fournisseur, persistant l'ensemble des modifications lors de la sauvegarde.
- **Prise en compte des modifications manuelles dans `config.toml`** : suppression de la réattribution inconditionnelle du modèle/fournisseur depuis la dernière session au démarrage. `config.toml` redevient la source de vérité au lancement de Spiritty, la restauration d'une session étant réservée à l'option explicite `-c` / `--continue`.
- **Synchronisation automatique des modèles recommandés** : `Config::load` fusionne désormais automatiquement les nouveaux modèles populaires (comme `gemini-3.8-flash`) en tête de liste dans les fichiers de configuration existants, tout en conservant les modèles personnalisés de l'utilisateur.
- **Ajout de modèle personnalisé dans la modale de configuration (`F2`)** : la confirmation d'ajout (`Enter`) ferme proprement le sous-menu de saisie pour afficher directement le modèle sélectionné, et la sauvegarde (`Ctrl+S` / `F2`) enregistre automatiquement le modèle dans le tableau `models` du fournisseur.

---

## v0.7.0 — 2026-09-03

### Ajouté

- **Multi-onglets interactifs dans le terminal split-screen (`Ctrl+T` / `Ctrl+W` / `Ctrl+Tab` / `Alt+1..9`)**
  — prise en charge native de plusieurs sessions PTY et serveurs en parallèle dans le panneau droit :
  - **Gestion des onglets** : création rapide avec `Ctrl+T` (ou clic sur `[+]`), fermeture avec `Ctrl+W` (ou clic sur `×`), navigation séquentielle `Ctrl+Tab` / `Ctrl+Shift+Tab` / `Ctrl+PgUp` / `Ctrl+PgDn`, et sélection directe `Alt+1..9`.
  - **Barre d'onglets dynamique & indicateurs** : affichage de chaque onglet avec son titre contextuel (`1: 💻 local`, `2: 🌐 vps-prod`, etc.) et pastille `●` en cas d'activité ou de sortie en arrière-plan.
  - **Synchronisation contextuelle avec l'agent IA** : le basculement d'onglet met à jour immédiatement le contexte système actif (`SSH`, `Docker`, `local`, dossier de travail, branche Git), assurant que l'agent IA assiste toujours l'environnement visible à l'écran.
  - **Multi-tâches non-bloquant** : chaque onglet maintient son propre processus PTY et son écran virtuel VT100 en arrière-plan sans bloquer l'interface.

- **Persistance et restauration du mode d'approbation (`auto_approve`) par session**
  — le niveau d'approbation automatique actif (`Safe`, `Sudo`, `YOLO`, `Off`) est désormais sauvegardé avec la session (`~/.config/spiritty/sessions/`) et restauré fidèlement lors d'un `spiritty -c` ou du chargement d'une session via `Ctrl+H`. La modale des sessions affiche l'indicateur visuel associé et les options en ligne de commande (`--yolo`, `--safe`, `--auto-approve <lvl>`) conservent la priorité absolue en cas de reprise forcée.

- **Prise en charge native du flux de réflexion Gemini 3.7 / 2.5 (`thought: true`)**
  — encapsulation automatique des chunks de réflexion du fournisseur Google Gemini en blocs `<think>…</think>` pour un rendu visuel repliable et animé identique aux modèles DeepSeek / OpenAI.

- **Journalisation automatique des plantages et paniques dans `~/.config/spiritty/crash.log`**
  — enregistrement systématique des erreurs fatales avec horodatage et backtrace complet.

- **Édition et gestion de fichiers distants en session SSH & conteneurs (`tool:read_file` / `edit_file` / `write_file`)**
  — les outils de fichiers dédiés fonctionnent désormais de manière transparente et sécurisée
  sur les machines distantes lors d'une session active `SSH`, `Docker` ou `Podman` :
  - `tool:read_file` : lecture distante via pipeline `base64` silencieux dans le flux PTY avec
    décodage mémoire et gestion du plafond de sécurité (100 Ko).
  - `tool:edit_file` : lecture préalable du fichier distant, validation stricte de l'unicité
    du fragment `old_string` en mémoire côté Rust, et réécriture atomique via `base64 -d | [sudo tee]`.
  - `tool:write_file` : écriture/écrasement complet distant avec encodage Base64 et élévation `sudo tee`
    automatique sur les chemins système (`/etc/`, `/var/`, `/usr/`, etc.).
  - Préservation de la sécurité human-in-the-loop : classification des risques (`Safe`, `Standard`,
    `Sudo`, `Risky`) et demande de confirmation utilisateur respectées sur tous les chemins distants.

- **Détection automatique des invites interactives et bascule de focus (`[y/n]`, confirmation, mots de passe)**
  — extension du détecteur de flux PTY pour identifier non seulement les demandes `sudo`/mots de passe, mais également les confirmations interactives courantes (`[y/n]`, `[o/n]`, `(yes/no)`, `Press [Enter] to continue`, `Are you sure you want to continue connecting`). Lorsque l'agent IA déclenche une commande nécessitant une réponse humaine, le focus clavier bascule automatiquement sur le terminal avec un toast explicite et le délai d'inactivité est augmenté à 120s pour laisser le temps à l'utilisateur de répondre.

- **Indicateur d'exécution en cours et invite d'interaction (`⚡ En cours · Shift+Tab`) dans le terminal**
  — lors de l'exécution d'un outil par l'agent IA, la barre d'en-tête du terminal affiche désormais un badge visible signalant qu'une commande est active et rappelant le raccourci universel `Shift+Tab` pour basculer instantanément dans le shell et interagir.

### Corrigé

- **Défilement automatique en bas du terminal lors de l'injection d'outils et commandes**
  — si le terminal était défilé vers le haut (historique), l'affichage du terminal restait figé sur les anciennes lignes pendant l'exécution des commandes d'outils de l'agent. Le défilement est désormais automatiquement remis à zéro (`reset_scroll`) dès qu'une commande est injectée, rendant immédiatement visible la sortie en direct.

- **Interruption propre des processus PTY suspendus lors de l'arrêt de génération (`Esc` / `Ctrl+C`)**
  — lorsqu'une commande bloquait dans le terminal (par exemple un `ssh` en attente ou un script interactif), appuyer sur `Esc` ou arrêter la génération annulait la tâche dans Spiritty mais laissait le processus actif en arrière-plan dans le terminal. Spiritty injecte désormais un signal d'interruption `\x03` (SIGINT) au PTY pour tuer proprement la commande et rendre immédiatement le prompt à l'utilisateur.

- **Correction du raccourci de fermeture d'onglet et préservation de l'effacement de mot (`Ctrl+W`)**
  — dans le terminal et l'entrée de chat, la combinaison `Ctrl+W` sert traditionnellement à effacer le mot précédent (`werase`). L'interception globale de `Ctrl+W` provoquait la fermeture intempestive de Spiritty quand un seul onglet était actif. Le raccourci de fermeture d'onglet est désormais `Ctrl+Shift+W` (ou clic sur `×`), `Ctrl+W` assure l'effacement de mot, et la fermeture d'onglet ne quitte plus l'application lorsque le dernier onglet est actif.

- **Isolation des scripts multi-lignes et commandes contenant `exit` / `set -e` dans un sous-shell (`bash -c '...'`)**
  — lorsqu'une proposition de script IA contenait `exit 1` ou `set -e` (par exemple un script de test avec `if [ -z "$KEY" ]; then exit 1; fi`), son exécution directe dans le shell interactif tuait le processus racine du PTY (`$SHELL`), provoquant la fermeture subite de Spiritty. Ces scripts sont désormais automatiquement encapsulés dans un sous-shell isolé, préservant la session interactive et capturant proprement la sortie et le code de retour sans quitter Spiritty.

- **Support complet du balisage d'outils DSML DeepSeek (`<skill>`, `<command>`)**
  — les modèles DeepSeek émettant des appels d'outils XML personnalisés sont désormais correctement interprétés comme des propositions de commandes interactives et leurs balises techniques sont filtrées de la vue de chat.

- **Robustesse du découpage des blocs de raisonnement et variantes de balises (`</thunk>`, `</thought>`, `</thinking`, `</th`)**
  — certains modèles de raisonnement (DeepSeek, GLM, Grok) émettent parfois des variantes de balises de fin de pensée (typo `</thunk>`, `</thought>`, `</thinking` sans chevron fermant ou coupure partielle `</th`) tout en plaçant un `</think>` fermant à la toute fin du message après l'appel d'outil. L'analyseur considérait l'ensemble du message (y compris le texte de réponse et l'invocation d'outil DSML) comme faisant partie de la réflexion privée, masquant la réponse et laissant la TUI figée sur *Deep thinking*. L'extraction des pensées et le découpage des outils gèrent désormais toutes ces anomalies de formatage et garantissent l'extraction immédiate des propositions de commandes.

- **Prise en charge des phrases d'approbation composées (`oui vas y`, `ok vas y`, `oui stp`) et déblocage des outils**
  — lorsqu'une demande d'approbation d'outil était en attente, les expressions composées courantes comme `oui vas y` ou `ok vas y` n'étaient pas reconnues comme une validation, et l'envoi d'un nouveau message laissait la tâche d'arrière-plan bloquée sur l'attente du consentement. L'analyseur d'approbation naturelle gère désormais toutes les locutions courantes et libère proprement la tâche en cours si une nouvelle directive est saisie.

- **Rétablissement de l'indicateur universel `💭 Deep thinking…` et du shimmer de réflexion active**
  — l'animation de pensée et le chronomètre de réflexion en temps réel restent visibles pendant toute la durée du calcul du modèle (y compris avant la réception du premier token et pendant le déroulement de la réflexion).

- **Résolution des timeouts SSE (passage à 45s connexion / 90s flux) pour les modèles de raisonnement**
  — les modèles raisonneurs (DeepSeek-R1 / V3, Gemini 3.7 Thinking, Claude 3.7 Thinking, o3-mini) et les longues sessions sous forte charge provoquaient des erreurs prématurées `Délai d'inactivité de 25s dépassé sur le flux du modèle (timeout SSE)`. Les timeouts ont été portés à 45s pour la connexion et 90s pour le streaming de pensée sur tous les fournisseurs (OpenAI, DeepSeek, Gemini, Anthropic, Ollama).

- **Optimisation du compactage de contexte LLM pour les très longues sessions (200+ tours)**
  — les sessions volumineuses saturaient le budget de tokens et allongeaient le TTFT :
  - **Filtrage des erreurs transitoires** : suppression automatique des messages d'erreur résiduels (`⚠️ Erreur : ...`) lors de la préparation de la conversation envoyée à l'API.
  - **Écrêtage des sorties géantes de commandes** : les sorties brutes volumineuses (> 6 000 caractères) sont automatiquement résumées avec préservation du début et de la fin de la sortie (`[sortie tronquée pour le contexte LLM]`).
  - **Plafonnement de la synthèse d'historique** : limitation du résumé des tours anciens à 25 points clés pour garantir une latence minimale.

## v0.6.4 — 2026-08-30

### Corrigé

- **Résolution des variables d'environnement des clés API au lancement GUI**
  — lors du lancement de Spiritty via un lanceur d'applications de bureau (sans passer
  par un terminal interactif existant), les clés d'API déclarées dans `~/.zshrc` ou
  `~/.bashrc` (`export GEMINI_API_KEY=...`, `DEEPSEEK_API_KEY`, etc.) n'étaient pas
  chargées car la sonde exécutait le shell en mode login non-interactif (`-l`). La sonde
  exécute désormais le shell en mode login interactif (`-l -i`) avec délimiteurs étanches,
  garantissant le chargement transparent des clés configurées dans votre shell rc.
- **Script d'installation (`install.sh`) : chemin absolu de l'exécutable dans le lanceur XDG (`Exec`)**
  — le fichier `spiritty.desktop` généré contenait `Exec=spiritty` relatif. Lorsque Spiritty
  est installé dans `~/.local/bin` (installation utilisateur sans sudo), les lanceurs de bureau
  et émulateurs de terminal (Ghostty, etc.) échouaient avec l'erreur `Failed to find executable spiritty`
  car `~/.local/bin` n'est pas présent dans le `$PATH` global de la session graphique. Le script
  utilise désormais le chemin absolu exact `${INSTALL_DIR}/${BINARY_NAME}`.

## v0.6.3 — 2026-08-30

### Ajouté

- **Script d'installation (`install.sh`) : détection de bureau et création du lanceur XDG**
  — sur Linux, l'installateur détecte désormais l'environnement de bureau actif
  (GNOME, KDE Plasma, XFCE, Hyprland, Sway, DankMaterialShell / DMS, etc.) et
  propose interactivement d'installer :
  - L'icône SVG dans `~/.local/share/icons/hicolor/scalable/apps/spiritty.svg`
  - Le lanceur `~/.local/share/applications/spiritty.desktop`
  - L'actualisation automatique des bases de données de lanceurs et de caches
    d'icônes (`update-desktop-database`, `gtk-update-icon-cache`, et redémarrage
    du service `dms` si actif).
- **Packaging CI (`release.yml`)** : l'archive tarball release inclut désormais
  l'icône `assets/icons/spiritty.svg`.

## v0.6.2 — 2026-08-30

### Ajouté

- **Icône SVG officielle embarquée + module `brand`** — l'icône « lampe à
  génie » `assets/icons/spiritty.svg` est poussée comme asset de marque du dépôt
  et **embarquée** dans le binaire via `include_str!` (nouveau `src/brand.rs` :
  `BRAND_GLYPH`, `brand_title()`, `ICON_SVG`). Le titre du panneau chat utilise
  `brand::brand_title()` et toute l'app partage un seul marqueur. La TUI
  n'affiche pas le SVG (un terminal ne peut pas dessiner un vecteur) — l'emoji
  🧞 reste le marqueur in-TUI.
- Documentation alignée sur le glyphe 🧞 (install.sh, README, README.fr,
  ROADMAP).

## v0.6.1 — 2026-08-30

### Modifié

- **Emoji de marque : fantôme → lampe bleue 🧞** — le fantôme `👻` devient la
  **lampe bleue 🧞** (référence à la « lampe à génie »), partout dans l'app :
  titre du panneau chat, préfixe des réponses assistant, aide CLI, rapports
  Markdown exportés et résumés de session.

## v0.6.0 — 2026-08-30

### Ajouté

- **Collage d'images / screenshots pour les modèles vision (`Ctrl+Shift+V`)**
  — lecture d'image depuis le presse-papiers (arboard `get_image` → pixels RGBA →
  PNG → base64), attachée au prochain prompt utilisateur. Le flux :
  - `Ctrl+Shift+V` déclenche une lecture asynchrone (thread dédié, garde
    anti-empilement) et stocke l'image dans `pending_image` — le toast
    « 🖼️ Image attachée… » confirme, puis l'image est jointe au prochain envoi.
  - `ChatMessage.attachments` (`Vec<MessageAttachment>`, `mime_type` +
    `data_base64`) transporte l'image ; `.data_uri()` expose la forme
    `data:<mime>;base64,…`. ⚠️ rétro-compatible : les sessions JSON sans la clé
    `attachments` se désérialisent toujours (attribut `#[serde(default)]`).
  - `prepare_conversation` conserve désormais un tour « image seule » (texte
    vide + pièce jointe) au lieu de le jeter comme vide — le provider reçoit
    bien la capture.
  - **Trois providers vision** : `openai.rs` (bloc `image_url` + data-URI),
    `gemini.rs` (part `inline_data`, base64 sans préfixe), `anthropic.rs`
    (bloc `image.source` base64). Le texte reste un part `text` pour satisfaire
    les API qui refusent un tour 100 % image.
  - Nouvelles dépendances : `base64` et `image` (feature `png`).
  - Tests : encodage PNG RGBA + base64, schéma JSON des trois blocs vision,
    data-URI, et survie d'un tour attachment-seule dans `prepare_conversation`.

- **Aperçu TUI de l'image collée (half-blocks)** — l'image en attente est
  rendue en direct au-dessus de la zone d'input par half-blocks (`▀` = 2 pixels
  par cellule, haut = fg, bas = bg), downsampling nearest-neighbour
  (letterboxé, ratio conservé, ~32×8 cellules). Aucun widget image ni dépendance
  ajoutée. Une ligne de statut affiche les dimensions et les raccourcis :
  `[Entrée] l'envoie` · `[Ctrl+Shift+⌫] retire`. `pending_image` porte
  désormais un `PendingImage` (le `MessageAttachment` pour l'envoi + les pixels
  RGBA décodés une fois au collage, aucun re-décodage par frame). Alpha
  aplati sur un fond sombre. Tests `render_halfblock_marks_cells…` /
  `flatten_over_dark…`.

- **`Ctrl+V` intelligent image/texte + lecture image Wayland réparée** — le
  collage image se fait désormais via **`Ctrl+V`** (global, actif des deux
  panneaux), et non `Ctrl+Shift+V` que Ghostty et la plupart des émulateurs de
  terminal capturent avant l'app. Trois corrections :
  - **Feature `wayland-data-control` activée** sur `arboard` (tire
    `wl-clipboard-rs`) — sous Wayland, `arboard::get_image()` échouait
    silencieusement faute de cette feature (arboard retombait sur le backend
    X11, qui ne voit pas la copie Wayland), d'où le collage du chemin au lieu
    de la vignette.
  - **`Ctrl+V` déplacé dans `handle_key`** (global) au lieu du seul paneau chat :
    en focus Terminal il était renvoyé tel quel au PTY, injectant le chemin de
    l'image dans le shell (local ou **SSH distant**) — pire sur un VPS.
    `handle_terminal_key` n'envoie plus `Ctrl+V` au PTY.
  - **`spawn_smart_paste_request`** : lit le presse-papiers une fois (arboard),
    priorité à l'image (`get_image`) → `PasteImage` (aperçu), sinon texte →
    `Paste` (collé dans le paneau actif). Diagnostics confirmés : le presse-
    papiers contient à la fois des pixels d'image (392×575) et un URI de fichier
    — l'image est bien lue en priorité.
  - **`Ctrl+Shift+V` capté par le terminal = image quand même** : Ghostty
    convertit `Ctrl+Shift+V` en un paste **texte** (l'URI/chemin du fichier).
    `handle_paste` détecte désormais que le texte collé est un chemin/URI
    d'image (`looks_like_image_path`) et relit le presse-papiers pour attacher
    l'image au lieu de coller le chemin dans le paneau (pire sur un VPS).
    Nouvel événement `PasteInto` (insertion sans re-détection) pour éviter toute
    récursion. Tests `image_path_detection`.

### Ajouté

- **Outils d'édition de fichiers dédiés** (`tool:read_file` / `tool:edit_file` /
  `tool:write_file`) — le modèle se rabattait sur des pipelines `sed`/`awk`/
  heredoc pour modifier un fichier, source de corruption (heredocs mangés) et
  d'erreurs 127 sur les sessions réelles. Trois outils structurés, dispatchés
  dans la boucle d'outils (`src/agent/mod.rs`) et documentés dans le system
  prompt (`src/agent/prompt.rs`) :
  - `tool:read_file` : affiche le fichier (tronqué à 100 Ko avec compteur), auto-approuvé (lecture seule).
  - `tool:edit_file` : remplacement d'une chaîne exacte unique (`old`→`new`, séparateur `---`), atomique — échoue bruyamment si le texte est introuvable **ou** apparaît plusieurs fois, au lieu d'appliquer un substitut partiel.
  - `tool:write_file` : écriture/écrasement d'un fichier complet, contenu verbatim (jamais d'ellipse/placeholder).
  - Le parseur (`parse_file_edit_fence`, `src/agent/tools.rs`) refuse un `edit_file` avec `old_string` vide et ignore les fences dans les blocs `<think>`.
  - **Classification par chemin** (`classify_file_edit`, `src/agent/safety.rs`) :
    fichiers utilisateur/`~/.config`/projets → Standard (auto-approuvé), `/etc`,
    `/usr`, `/var`, `/root`, `/boot`... → Sudo (demande confirmation hors niveau Sudo),
    chemins sensibles (`.ssh`, `.zshrc`, `fstab`, `sudoers`, `ssh` config) → Risky
    (uniquement auto-approuvé en YOLO).
  - Tests : parsing des fences (read/write/edit, multi-lignes, `<think>`, old vide),
    classification par chemin, et round-trip lecture/écriture/édition (replacement
    unique, ambigu, introuvable).
  - **Refus en session distante (SSH/container)** : les outils d'édition agissent sur
    le système **local** de Spiritty, pas sur le serveur distant. La boucle d'outils
    détecte désormais la session via `sys_ctx.active_session` (SSH/container) et refuse
    `read_file`/`edit_file`/`write_file` avec un message explicite invitant à revenir
    aux commandes shell (`cat`/`sed`/`tee`/heredoc/`scp`) via `tool:run_command` —
    plutôt que d'éditer silencieusement un fichier local qui n'est pas celui que
    l'utilisateur visualise. Règle ajoutée au system prompt.

### Corrigé

- **Détection SSH depuis le panneau shell : faux `Ssh` forgé sur cible invalide**
  — `parse_ssh_args` (`src/system/process_watcher.rs`) acceptait n'importe quel
  premier token non-flag comme cible ssh. Un processus de premier plan dont
  `argv[0]` se résout en `ssh` mais avec un argument non-cible (durée/`sleep`,
  numéro isolé, `-N` sans destination, reaper) produisait un faux
  `ActiveSession::Ssh { target: "30", host: "30" }`, faussant la détection
  Local↔SSH en course. Ajout d'une validation stricte `is_valid_ssh_host` : la
  cible doit être un hostname (lettres/chiffres/`.`/`-`/`_`, pas purement
  numérique), une IPv4, ou une IPv6 (entre `[]` ou avec `:`). Tests
  `test_ssh_without_valid_target_is_rejected` / `test_ssh_with_valid_targets_is_accepted`.

- **Modal de reconnexion SSH proposé à tort sur une session locale** — le cas
  où le modèle *émet* une commande `ssh …` (proposition/exemple) faisait
  apparaître la modale de reconnexion au rechargement d'une session en fait
  locale. L'heuristique `extract_ssh_target` (`src/session/mod.rs`) considérait
  tout message `💻 \`ssh …\`` comme preuve d'une session distante, alors qu'il
  ne prouve rien (la commande peut avoir simplement tourné, ou le `ssh` suivi
  d'un `exit`). La seule preuve fiable qu'une session s'est terminée sur un
  shell distant est le **reste de prompt** (`user@host:~$`) : le signal
  « commande `ssh` nue » est supprimé, l'inference ne se fait plus que par un
  prompt distant. Test `infers_target_from_ssh_command_message` renommé
  `bare_ssh_command_message_does_not_infer_target`, et `newest_message_wins`
  réécrit sur des prompts distants.


- **`<think><think>` imbriqués dans la réflexion affichée** (audit session
  20260830) — certains modèles raisonneurs (GLM/Z.ai/DeepSeek) émettent leur
  propre `<think>` **dans** le contenu visible, en plus du wrapper que Spiritty
  ajoute pour `reasoning_content` : l'extracteur de raisonnement
  (`extract_thought_block`, `src/ui/chat_panel.rs`) captait alors `<think>…`
  avec le tag littéral en tête. Le `thought` extrait est désormais passé par un
  nettoyage `strip_residual_reasoning_tags` qui retire tous les délimiteurs de
  raisonnement résiduels (`<think>`/`<thought>`/`<reasoning>` et leurs fermetures)
  — la délibération est affichée verbatim, le cas normal d'un simple bloc reste
  inchangé. Test `nested_think_keeps_reasoning_verbatim`.

- **Heredocs multilignes mangés par le line-editor interactif local (zsh/bash)**
  (audit session 20260830) — les scripts heredoc (`sudo tee … <<'EOF'` avec un
  corps sur plusieurs lignes physiques) étaient injectés bruts dans le PTY :
  l'éditeur de ligne du shell local découpait le fichier en plusieurs lignes et
  en perdait le corps (fragments « `cmdand heredoc> =` », `<<''EOF'>`,
  duplication de lettres constatée sur les sessions réelles). Désormais
  `format_command_for_pty_with_session` route les commandes contenant un heredoc
  (`<<`) dans `bash -c '…'` pour **tout shell local** (zsh/bash/dash, pas
  seulement fish) : le bloc entier devient UNE seule ligne logique pour l'éditeur
  interactif, et bash exécute le script verbatim. Les shells distants restent
  inchangés (leur éditeur gère le multiligne, le wrapping risquait d'en changer
  la sémantique), et la syntaxe bash « nue » sans heredoc reste native pour bash.
  Tests `format_command_for_pty` étendus (heredoc zsh local → wrap, heredoc
  remote → natif, bash-syntaxe non-heredoc → natif).

- **Propositions de commande extraites du `<think>` du modèle exécutées à tort**
  (audit session 20260830) — l'extracteur de propositions
  (`extract_all_command_proposals`, `src/app.rs`) scannait les fences de code
  au sein du bloc `<think>…</think>` du raisonnement, que le parseur d'appels
  d'outils retirait déjà (`strip_think_blocks`, `src/agent/tools.rs`) mais pas
  lui. Le raisonnement contient souvent des fences *d'exemple* non exécutables,
  qui devenaient des cartes ⚡ et étaient exécutées en lieu et place de la
  vraie commande : cas réel « automount `/dev/sdb1` » où seule la ligne de
  contenu du heredoc (`UUID=… /mnt/data …`) a été injectée (`code 127`) en
  place du `sudo mkdir … printf … | sudo tee -a /etc/fstab`, et cas `.desktop`
  exécuté comme commande. `extract_all_command_proposals` retire désormais les
  blocs `<think>`/`<thought>`/`<reasoning>` avant de scanner (réutilise
  `strip_think_blocks`), avec test de non-régression sur le contenu réel.

## v0.5.6 — 2026-08-29

### Performance

- **Rendu fenêtré du panneau chat : fini la rame à 100% CPU sur les longues
  sessions** — le rendu concaténait **toute** l'historique en un seul Paragraph
  et re-comptait ses lignes wraps à chaque frame pour calculer le scroll : sur
  une session de ~1000 messages / ~10 000 rangées, la vue bottom-ancrée
  re-wrappait tout ce qui précède la fenêtre visible à 11 fps, saturant un
  cœur CPU pendant la génération LLM. Pass B ne matérialise plus que les
  messages recouvrant la fenêtre visible (géométrie par message mémorisée en
  pass A) et le scroll est dérivé de la somme des hauteurs par message (toutes
  deux calculées par ratatui : la vieille divergence venait du compteur *simulé*
  depuis retiré). Mesure sur session de 9820 rangées : CPU de streaming
  100 % → ~20 %.

### Corrigé

- **Dépliage de la réflexion aléatoire au clic** — après un toggle réussi, les
  zones de clic (« 💭 Réflexion · ») étaient vidées jusqu'au prochain rendu
  (~90 ms) : un second clic rapide (double-clic, clics rapprochés) tombait sur
  une liste vide et démarrait une sélection de texte au lieu de basculer.
  Les zones ne sont plus vidées manuellement (le rendu les recalcule de toute
  façon à chaque frame) et la cible est élargie à 2 rangées quand la réflexion
  est pliée (la rangée vide sous le toggle appartient à la cible ; en état
  déplié, elle reste sélectionnable). Instrumentation de diagnostic
  `SPIRITTY_UI_DEBUG=1` (`/tmp/spiritty_ui_debug.log`).

## v0.5.5 — 2026-08-29

### Modifié

- **System prompt : une seule proposition de commande par réponse** — le §2
  enseignait littéralement aux models d'émettre un bloc bash **par commande**
  pour les « multi-step plans » : les models empilaient les étapes successives
  en cartes Alt+1/Alt+2/Alt+3 à déclencher à l'aveugle. Désormais : une
  proposition par réponse pour les étapes séquentielles (le résultat revient au
  model avant l'étape suivante) ; plusieurs cartes uniquement pour des
  **alternatives** d'une même action (pacman/apt/dnf → Alt+1/2/3) ; chaînage
  `&&` pour les étapes trivialement atomiques. Synchronisé dans le prompt
  intégré (`prompt.rs`) et le template par défaut (`config/mod.rs`).

### Corrigé

- **Appels d'outils émis en tag HTML par Gemini ignorés** — certains models
  écrivent `<tool:run_command>` (tag XML, fermant omis, ``` isolé) au lieu du
  bloc fencing ```` ```tool:run_command ```` enseigné : la commande n'était
  jamais exécutée, le model se penait sur son propre format puis affichait un
  *exemple* de syntaxe (💻 `commande`) que la politique d'auto-approbation
  exécutait tel quel (`code 127`). Le parseur reconnaît désormais les tags
  HTML-style (fermé, avec corps fence, ou tronqué) et n'extrait plus jamais
  d'appel d'outil depuis les blocs `<think>` (le raisonnement n'est pas une
  action). Règle system prompt ajoutée : syntaxe de bloc stricte + interdiction
  des commandes factices/plageholders dans les exemples.
- **Gemini : HTTP 400 « Requests ending with a model turn are not supported »** —
  chaque requête embarquait le placeholder `Assistant("")` créé par l'UI comme
  cible de streaming : l'historique se terminait donc par un tour `model`, que
  l'API Gemini refuse (OpenAI-compatible tolère, d'où le passage inaperçu). La
  préparation de la conversation (`prepare_conversation`) droppe désormais tous
  les messages vides, tous rôles confondus ; le provider Gemini fusionne en plus
  les contenus consécutifs de même rôle (résumés System mappés `user`, résultats
  d'outils user/user) pour une requête canonique.
- **Capture PTY : plus de timeout de 45s sur les commandes locales** — le scan
  incrémental du sentinel de fin de commande (`OSC 777`) n'examinait que les
  20 derniers caractères du buffer après chaque chunk. Quand l'echo + la sortie +
  le sentinel coalescent dans une seule grosse lecture PTY (fréquent sur shell
  local rapide, selon le scheduling), le sentinel passait inaperçu et la capture
  attendait l'expiration du timeout dur. Le scan utilise désormais un watermark
  (`sentinel_scan_upto`) qui rescanne uniquement le recouvrement nécessaire :
  détection en ~4 ms au lieu de 45 s, coût amorti O(nouveaux octets) conservé.

### Ajouté

- **Instrumentation de capture** (`SPIRITTY_CAPTURE_DEBUG=1`) : journal
  d'événements du cycle de capture (armement, sighting du sentinel, conclusion,
  dump du buffer à 5 s) dans `/tmp/spiritty_capture_debug.log` pour diagnostiquer
  après coup les captures qui n'aboutissent pas.

## v0.5.4 — 2026-08-28

### Changé

- **CI release : retrait de la cible `x86_64-apple-darwin`** — les runners macOS
  Intel hébergés (`macos-13`) sont retirés par GitHub ; le job restait bloqué en
  file d'attente (0 step, aucun runner assigné) sans jamais produire de binaire.
  La matrice ne build plus que Linux (`x86_64` + `aarch64`) et macOS Apple
  Silicon (`aarch64`).

### Corrigé

- **Footer : affichage du `Ctx` corrigé** — `get_context_used_tokens` estimait
  l'historique complet de la session (rémanence de l'option C), d'où un
  « Ctx: 178k / 131k (100%) » absurde (utilisé > fenêtre, clampé à 100%) et quasi
  identique au compteur de tokens total. Le `Ctx` estime désormais le contexte
  **compacté réellement envoyé** au modèle (résumé + 8 derniers tours verbatim),
  cohérent avec la compaction à la requête ; le compteur « tok » reste le total
  de la session.

## v0.5.3 — 2026-08-28

### Changé

- **System prompt : interdiction d'abréger les commandes avec `...` ou un placeholder** —
  ajout d'une règle explicite dans `IMPORTANT RULES` : toujours coller le contenu
  intégral d'un heredoc/script/fichier dans le bloc de code, jamais `...` /
  `BASE64` / `[content]` comme raccourci (le bloc est exécuté tel quel — un
  placeholder est écrit verbatim sur disque ou échoue ; il n'y a pas de
  troncature côté outil). Corrige le cas où GLM (`glm-5.3-flash`) réduisait ses
  commandes longues à `...`, puis attribuait à tort la casse à une
  « troncature client ».

## v0.5.2 — 2026-08-28

### Changé

- 🟢 **Historique complet persisté, compactage réduit au contexte LLM** (option C,
  « quand on remonte dans l'historique on ne voit plus la globalité des échanges ») :
  `save_current_session` ne compacte plus — le JSON de session conserve **tous**
  les échanges, et le rechargement restaure la globalité de la conversation (fini
  le plafond « 1 résumé + 8 tours = 9 messages » hérité du compactage à la
  sauvegarde). Le compactage déménage **au moment de la requête** :
  `agent::send_prompt` applique désormais `compact_chat_messages` (extraction de
  la logique de `Session::compact` en fonction pure dans `session/mod.rs`) — les
  tours anciens roulent dans un résumé System et les 8 plus récents partent
  verbatim au fournisseur, donc le budget de contexte reste borné sur les
  sessions longues, avec un bénéfice immédiat : le contexte live est compacté à
  chaque tour, pas seulement au rechargement. Les sessions JSON déjà compactées
  par les versions précédentes restent telles quelles (l'historique perdu avant
  cette version n'est pas reconstituable). Validé E2E dans un HOME isolé :
  session de 15 messages → rechargement → re-sauvegarde → 15 messages sur
  disque, aucun résumé persisté.

### Documentation

- README.md / README.fr.md rafraîchis pour v0.5.x : le compactage est documenté
  honnêtement (historique complet persisté et restauré — plus de plafond à 9
  messages dans la liste des sessions —, et compactage limité au seul contexte
  LLM : résumé structuré + 8 derniers tours verbatim), badges de classification
  en 4 niveaux (🟢 Safe / 🟡 Standard /
  🟣 Sudo / 🔴 Risky), modale de reconnexion SSH (`⏎ Se reconnecter`) et hint
  `· 🔗 SSH (reprise)` ajoutés à la section SSH, raccourcis `F10` (autorisation
  en un appui) et `F6` (focus) dans la table, et ASCII-art corrigé
  (`Ctrl+Espace` pour le focus, `Alt+N` pour exécuter — l'ancien
  « Enter: Execute | Tab: Edit » ne correspondait à aucun binding actuel).

## v0.5.1 — 2026-08-28

### Corrigé

- 🔴 **La modale de reconnexion SSH n'apparaissait pas au rechargement in-app**
  (rapport « quand je charge [la session SSH] je n'ai pas la modal qui me propose
  de me log en ssh ») : `load_session` posait bien l'offre `SshReconnect`, mais le
  handler de la liste des sessions la **refermait immédiatement** (`modal = None`
  après l'action `Load`) — l'offre ne survivait que sur le chemin `-c`
  (démarrage), où rien ne la refermait ensuite. La liste se ferme désormais
  **sauf si** l'offre de reconnexion vient d'être posée. Validé de bout en bout
  sur une vraie session SSH rechargée in-app (modale « ⏎ Se reconnecter /
  Esc Plus tard » affichée, titre `· 🔗 SSH (reprise)` restauré).

## v0.5.0 — 2026-08-28

### Ajouté

- **Indicateur de session SSH persistant** : le panneau terminal affiche déjà
  `🌐 SSH: <cible>` quand une session distante est détectée ; il affiche désormais
  aussi **`· 🔗 SSH <cible> (reprise)`** quand on reprend une session avec `-c` qui
  **était en SSH** alors que le PTY est encore local (avant de se reconnecter). La
  cible SSH est persistée dans le JSON de session (`last_ssh_target`, sticky — une
  session qui a été distante reste marquée), et pour les sessions créées par une
  version antérieure une heuristique conservative la déduit de l'historique (commande
  `ssh …` explicite, prompts distants classiques `user@host:~$`/`user@host$` ou style
  zsh à crochets `[user@host:/chemin] ±` — avec garde-fous anti git-remotes/e-mails).
  Le titre se dégrade proprement selon la largeur du panneau (mesurée en cellules,
  pas en octets) : `· 🔗 SSH <cible> (reprise)` → `· 🔗 <cible> (reprise)` →
  `· 🔗 SSH (reprise)` → `· SSH (reprise)` — le hint prime sur le cwd/branche quand
  la place manque. Le toast de restauration mentionne aussi la reprise SSH. Rien ne
  s'affiche pour une session 100 % locale.
- **Modale de reconnexion SSH** : au chargement d'une session `-c` qui était en SSH
  alors que le PTY est local, une petite modale centrée propose **`⏎ Se reconnecter`**
  à l'hôte enregistré (l'injection `ssh <cible>` dans le terminal + focus automatique)
  ou **`Esc Plus tard`** (le hint du titre reste affiché). Jamais proposée si déjà
  connecté ou pendant une capture PTY en cours ; i18n FR/EN.
  La cible inférée depuis un reste de prompt (`user@hostname`, ex. `xorne@prod`) est
  **résolue en adresse connectable** via le store d'hôtes (`prod` → `ducasse-seine.com`,
  profil le plus récent en cas de collision, utilisateur conservé s'il diffère) — et le
  hint persisté est auto-corrigé pour les prochaines reprises.
- **Thinking sur une seule ligne + timer de réflexion + clic pour déplier** :
  - Le raisonnement du modèle s'affiche désormais **une seule ligne** (`▸ 💭 Think · …` /
    `▸ 💭 Réflexion · …`), montrant la **fin** du raisonnement (tronqué avec `…` si besoin)
    au lieu du bloc multi-lignes qui poussait la réponse hors écran.
  - **Ligne « 💭 Deep thinking… / Réflexion profonde… » conservée SOUS la ligne
    Think** pendant tout le déroulé du raisonnement, avec le **timer `mm:ss` en
    direct** (shimmer cyan animé) ; le timer disparaît dès que la réponse commence.
  - **Clic sur la ligne** `▸/▾` pour **déplier/replier** le raisonnement complet. Le
    hit-testing s'appuie sur les rangées autoritatives de ratatui (pas de dérive de wrap),
    et le bouton bascule l'état par message (cache invalidé proprement, fin toujours
    visible).
- **F10 = Autoriser la commande en attente** : la validation `ok`/`oui` + Entrée devient
  une simple touche **F10**, opérante depuis les deux panneaux (chat et terminal) — la
  carte ⚡ affiche désormais `[F10] Autoriser · [oui/ok + ↵] · [Esc] Refuser`. Pensé pour
  les longues sessions d'audit où l'approbation répétée devient fastidieuse.
- **Wording « Réflexion profonde » animé** : pendant la phase silencieuse de réflexion d'un
  modèle (aucun token reçu), le panneau chat affiche une ligne dédiée
  `⟳ 🧞 💭 Réflexion profonde…` / `⟳ 🧞 💭 Deep thinking…` parcourue d'un **shimmer
  dégradé cyan** (vague de lumière DarkGray→LightCyan balayant le texte à chaque tick,
  avec pause aux extrémités) au lieu du seul ghost muet.
- **Édition complète du prompt multi-lignes** :
  - `↑` / `↓` naviguent désormais les rangées visuelles du champ prompt (lignes logiques
    ET retours à la ligne soft-wrappés d'un paragraphe collé), en préservant la colonne
    curseur (clampée à la largeur de la rangée cible). L'historique du prompt ne prend la
    main qu'aux extrémités (première/dernière rangée), comme dans un vrai éditeur.
  - `Ctrl+A` / `Ctrl+E` (mémoire readline) : début/fin de la **ligne logique courante**
    (pas seulement du buffer entier).
  - La zone de saisie suivait déjà le curseur verticalement (scroll borné à 8 lignes) —
    désormais le curseur peut enfin y arriver.
- Nouvelle segmentation partagée `prompt_visual_rows()` : source unique de vérité pour le
  placement du curseur (`compute_prompt_cursor_and_lines` refactoré dessus) et la
  navigation flèches — plus aucun risque de dérive entre affiché et édité.

### Corrigé

- 🔴 **Doublon de proposition de commande « Alt+N » vs « F10 »** (rapport avec
  capture : « il l'a proposé en alt+f1 et via f10 ») : les modèles à outils textuels
  écrivent souvent la commande **dans leur texte** (fence → carte interactive
  « ⚡ COMMANDE #N / Alt+N Exécuter ») **et** l'appellent via le protocole outil
  (→ carte « ⚡ DEMANDE D'AUTORISATION / F10 Autoriser ») — la même commande se
  retrouvait avec **deux affordances concurrentes**, et `Alt+N` pouvait exécuter en
  court-circuitant le consentement en cours. Dorénavant : la fence identique à la
  commande en attente est **démotée en snippet inerte** (le code reste visible, la
  carte d'autorisation devient l'unique surface d'action), et `Alt+N` est **bloqué**
  (toast explicite) tant qu'une autorisation est en attente **ou qu'une capture PTY
  est en cours** — une injection pendant une capture aurait de plus écrasé la
  capture active.
- 🔴 **Le timer après « 💭 Deep thinking… » disparaissait dès le premier outil**
  (rapport « le timer qui est après Deep thinking a disparu ») : l'`Instant` qui
  l'alimente est **consommé** (`take()`) par le calcul tokens/s au moment du
  tool call et n'était **jamais ré-armé** pour les tours de continuation — depuis le
  premier outil jusqu'à la fin de la génération, la ligne restait sans chrono.
  Ré-armé à chaque `AgentNewTurn` (continuation après un outil MCP/web/commande) et
  dans `record_command_result` (analyse après commande utilisateur, chemin sans
  événement), avec remise à zéro des compteurs de segment (tokens/s corrects par
  segment). Bug latent de la vague shimmer+timer, devenu visible maintenant que la
  boucle d'outils s'exécute à nouveau.
- 🔴 **Freeze complet de l'UI pendant une exécution d'outil** (rapport « l'app a
  freeze complet, j'ai kill le process mais j'ai toujours des sorties sur mon
  terminal ») — trois trous structurels corrigés ensemble :
  1. **Écritures PTY bloquantes sur la boucle UI** : chaque frappe, collage et
     injection d'outil écrivait *directement* dans le master PTY depuis le thread
     d'événements. Une écriture master **bloque** tant que l'aval ne consomme plus
     (pipe SSH calé, buffer d'entrée du tty distant plein, shell distant figé) — un
     seul write bloqué gelait toute l'application. Les écritures passent désormais
     par un **thread écrivain dédié** (file FIFO, `write_all` n'enqueue plus jamais,
     l'ordre \x15→commande→sentinelle est préservé) ; les réponses aux queries
     terminales (DA1/DSR/CPR/OSC) transitent par la même file.
  2. **`clean_pty_output` O(N) relancé à chaque tick** (~11×/s) sur le buffer de
     capture, qui n'avait **aucune limite de taille** : une commande bavarde rendait
     le coût par frame quadratique (UI progressivement figée). Le nettoyage n'est
     plus déclenché que quand la sortie s'est stabilisée ou au hard-timeout, avec
     un **cache indexé par la longueur** du buffer (tick sur buffer calme = gratuit),
     et la capture est **plafonnée à 1 Mio** : au-delà, les octets sont comptés sans
     être bufferisés, une petite queue roulante continue de détecter la sentinelle
     OSC 777 (match sur vrai octet ESC — l'écho `printf '\033]777…'` ne peut pas
     faux-positiver) et le résultat porte un avis explicite « ⚠️ Sortie tronquée ».
  3. **SIGTERM/SIGINT/SIGHUP sans restauration de terminal** : un `kill` externe
     quittait le tty en raw mode, curseur caché, écran alterné collé (d'où les
     « sorties résiduelles » nécessitant un `reset` manuel). Un hook de signaux
     (signal-hook) restaure raw-mode/écran/curseur/protocole kitty puis sort avec le
     statut conventionnel `128+signal` — même quand la boucle principale est figée.
     `kill -9` reste impossible à intercepter (`reset` reste la parade).
- 🔴 **Markup brut de tool-call affiché dans le chat** : certains modèles émettent
  leur appel d'outil en XML inline (`<tool_calls><invoke name="exec_command"><parameter
  …>…`) dans le flux visible au lieu du protocole d'outil. La commande était bien
  exécutée (carte + bloc de commande), mais le XML fuitait et s'affichait en gris
  sous la réponse. Ces blocs (`<tool_calls>`, `<DSML>`, `<invoke>`, `<parameter>`)
  sont désormais **strippés**, y compris les blocs non fermés (stream coupé en plein
  milieu), en préservant la réponse réelle et les blocs de code markdown. Le
  marqueur hybride est désormais **normalisé avant découpe** (`<｜｜DSML｜｜invoke …>`
  → `<invoke …>`, marqueur sans chevron → devient le chevron) : l'ancien ouvreur
  ne matchait qu'à la 2ᵉ pipe pleine-largeur et laissait un résidu `<｜` affiché
  (rapport « affichage de `< |` ») ; un fragment de balise tronqué en fin de message
  (`<`, `<|`, `</`) est aussi jeté, sans toucher aux `<` légitimes en plein texte.
- 🔴 **Écho corrompu reporté comme « sortie » de commande** (rapport « le modèle
  n'arrive pas à récupérer le résultat des commandes ») : sur session SSH, les
  redraws du line-editor distant injectent des fragments dans l'écho brut et
  **doublent des caractères en plein milieu** (`logs/` → `llogs/`, `…8ecd…` →
  `…8ecdd…`). Le nettoyeur d'écho exigeait une correspondance exacte avec la
  commande envoyée → l'écho corrompu survivait au nettoyage et était reporté au
  modèle comme la sortie réelle. Le modèle en déduisait que « le client altère
  systématiquement les commandes » (hallucination documentée dans son reasoning)
  et contournait avec scripts/heredocs au lieu de lire les vrais résultats.
  Désormais l'écho est aussi reconnu par **sous-séquence** (une corruption par
  insertions conserve tous les caractères envoyés dans l'ordre, comparaison sur
  projection alphanumérique, fenêtre de longueur ±50 %) : écho corrompu strippé,
  vraie sortie préservée, première ligne légitime non touchée. Un `grep` sans
  résultat reporte maintenant un honnête « ⚠️ Aucune sortie capturée » au lieu
  d'un faux écho exploitable.
- 🔴 **Appels d'outils DSML hybrides jamais exécutés** (rapport « le modèle ne fait
  plus rien ») : le modèle GLM/Z.ai a dérivé vers son échafaudage natif
  `<｜｜DSML｜｜invoke name="exec_command">` (pipes pleine-largeur U+FF5C) émis en
  texte, format que le parseur ne reconnaissait pas — aucun outil détecté, aucune
  carte de commande, le tour se terminait en silence. Le parseur (`parse_tool_call`)
  normalise désormais ces marqueurs (`｜`→`|`, retrait `|DSML|`) et extrait la
  structure HTML-style `<invoke name="…">` + `<parameter name="command">…</parameter>`
  (tolère les streams tronqués), pour `RunCommand` et `WebSearch`. Les formats
  existants (` ```bash `, `<tool_call>`, `<|tool_calls|>`, JSON) restent prioritaires
  et inchangés.
- 🔴 **Captures « vides-succès » sur commandes distantes lentes** : le settle (3 s de
  silence, ou 800 ms sur prompt réaffiché) pouvait conclure la capture pendant que
  **seul l'écho de la commande** était arrivé — la vraie sortie (latence SSH, handshake
  mysql…) atterrissait après, et le résultat reportait un mensongeux
  « Commande exécutée avec succès dans le terminal », sur lequel le modèle raisonnait
  (deux commandes consécutives à vide sur le VPS, rapport utilisateur). Désormais une
  capture nettoyée **vide** ne conclut jamais sur le settle : elle attend la vraie
  sortie jusqu'au hard-cap (45 s / 120 s), puis reporte un **avertissement explicite**
  (« ⚠️ Aucune sortie capturée… ») qui pousse le modèle à vérifier le terminal et
  relancer, au lieu d'inventer un succès. Le chemin sentinelle locale (code de sortie
  reçu) n'est pas concerné : là, « succès sans sortie » est véridique.
- 🔴 **Le thinking du modèle était invisible** (provider OpenAI-compatible) : le reasoning
  streamé par GLM / Z.ai / DeepSeek arrive dans le champ **`reasoning_content`** des chunks
  SSE — séparé de `content` — et Spiritty le **jetait silencieusement**. Le parser
  `<think>` de l'UI n'a donc jamais rien reçu : aucune ligne « Think », seulement le
  ghost shimmer. Le provider capture désormais `reasoning_content` et le ré-encapsule à
  la volée en bloc `<think>…</think>` dans le stream (machine à état `ReasoningBracket` :
  ouverture au premier chunk de reasoning, fermeture avant le premier chunk de réponse,
  reasoning parasite post-réponse ignoré) — l'UI existante l'affiche sans changement.
- ⚙️ **Timer de réflexion sur la même ligne** : pas de ligne dédiée — le temps
  `mm:ss` depuis la dernière interaction est appended **après le wording**
  (`💭 Deep thinking… 8m 29s` / `💭 Réflexion profonde… 8m 29s`), et sur la ligne
  think repliée après la fin du raisonnement (`▸ 💭 Think · <fin> · 8m 29s`).
- 🔴 **« Le décalage s'accentuait, je ne vois plus la fin des réponses » (y compris au
  relancement avec `-c`)** : `max_scroll` du panneau chat était calculé en sommant, par
  message, un nombre de rangées issu d'une **simulation maison** du word-wrap — qui diverge
  (sous-compte) du wrap réel de ratatui sur les tableaux markdown, cartes heredoc et glyphes
  larges. Chaque message accumulait son erreur : le bas de la conversation devenait
  **inatteignable par scroll** (l'offset saturait avant la fin), et la queue des réponses
  disparaissait sous le pli — reproduit et prouvé sur la session réelle de l'utilisateur.
  `max_scroll` et le badge de lignes dérivent désormais de **`Paragraph::line_count()`**
  (le même WordWrapper que le rendu, feature `unstable-rendered-line-info` activée) — un
  seul `Paragraph` sert au comptage et au rendu, zéro clone supplémentaire par frame.
  Tests end-to-end : fin de la dernière réponse visible dans le buffer peint, session
  rechargée comprise.
- 🔴 **Capture PTY potentiellement bloquée à l'infini** (`on_tick`) : quand la queue de sortie
  *ressemblait* à une invite de mot de passe (mot-clé `password:`/`passphrase` — y compris un
  faux positif, ex. écho d'un fichier de config contenant `password:`), le timeout passait à
  `u64::MAX` **et** le fallback de settle était désactivé — sur une session SSH distante sans
  sentinelle OSC, la capture ne se terminait **jamais** : l'agent restait bloqué en attente du
  résultat, UI figée, seule issue = relancer l'app. L'attente mot de passe reste généreuse mais
  est désormais bornée (`PASSWORD_WAIT_HARD_CAP_SECS = 120 s`).
- ⚡ **Lenteur des commandes distantes (SSH)** : chaque commande payait le fallback de settle
  (3 s de silence minimum) faute de sentinelle OSC côté remote. Nouveau **settle accéléré
  piloté par le prompt** : quand la queue de sortie se termine par une ligne ressemblant au
  PS1 réaffiché par le shell distant (`user@host:path$`, `#`, `>`…) et est silencieuse depuis
  800 ms, la capture se termine immédiatement (~1 s au lieu de ~3,8 s par commande) — c'est la
  moitié du temps perçu dans les boucles d'édition de fichiers distantes. Les commandes sans
  prompt final (streams, `tail -f`) gardent le fallback 3 s ; les shells locaux hookés
  continuent d'attendre leur sentinelle OSC 777.
- 🔴 **Bloc ```` ```bash ```` affiché « bash » + commande mais non exécutable** (reprise du
  bug historique, nouvelle variante) : l'heuristique anti-flèches (`->`) rejetait tout bloc
  dont une commande contenait une flèche **dans un titre quoté** — ex. réel :
  `echo "=== CLONE DB resa-v3 -> resa_pp ==="` (session prod Stripe/reséa). Le bloc était
  rétrogradé en boîte snippet étiquetée `bash`, donc sans raccourci d'exécution. Les tags
  shell explicites (`bash`/`sh`/`zsh`…) court-circuitent désormais les heuristiques de
  format (flèches, formulations conversationnelles) : le tag est une intention d'auteur ;
  les fences **non taguées** restent protégées par ces mêmes heuristiques.
- 🔴 **Flèches mortes dans `vi`/`vim` dans le panneau terminal** : le shell passait son
  clavier en mode application-cursor (DECCKM via `smkx`, `ESC[?1h`) et attendait les
  flèches en SS3 (`ESC O A`), mais Spiritty n'envoyait que du CSI (`ESC [ A`) — flèches OK
  dans le shell, mortes dans vi. L'état DECCKM du child est maintenant suivi en scannant
  son flux de sortie, et l'encodeur de touches bascule CSI↔SS3 en conséquence
  (flèches + Home/End).
- 🔴 **Régression « scroll de lignes vides pendant le stream »** (introduite par le cache de
  rendu, détectée au premier test utilisateur) : la queue assistant en cours de streaming
  n'était pas stockée dans le cache alors que la passe de clonage puise exclusivement dedans
  → ses rangées manquaient au widget mais restaient comptées par le scroll (`max_scroll`
  surdimensionné = défilement dans le vide), jusqu'au « réaffichage complet » à la fin du
  stream. La tail est maintenant stockée inconditionnellement — le prédicat de fraîcheur la
  re-compose déjà à chaque frame pendant `is_generating`, donc zéro stalence possible.
- Ghost de réflexion invisible pendant le stream : même cause racine (la ligne loader vivait
  dans la tail non-clonée).

### Performance

- **Cache de rendu par message pour le panneau chat** (chantier « virtualisation M4 », option A) :
  chaque frame ne re-parse plus TOUT l'historique (markdown, cartes ⚡, blocs réflexion,
  wrap-simulation) mais uniquement les messages réellement modifiés depuis la frame
  précédente.
  - Nouveau `ChatRenderCache` sur `App` (interne `RefCell`, thread UI uniquement) :
    artefacts rendus par message validés par clé de génération structurée
    `(largeur panneau · langue · mode debug · thème)` + rôle + longueur d'octet du contenu ;
    la queue en streaming est recomposée à chaud sans polluer le cache.
  - Extraction verbatim des branches System/User/Assistant en composeurs purs
    (`compose_single_message`) + carte d'approbation hors cache — zéro divergence visuelle
    vis-à-vis de l'ancienne boucle inline.
  - Passe B conservée en clonage complet depuis le cache : l'optimisation « fenêtre visible
    seule » a été tentée puis rejetée faute de pouvoir reproduire fidèlement le wrap paragraphe
    d'une ligne tronquée à sa tête (commentaire détaillé dans le code). Le gain majeur reste
    la suppression des re-parse/re-wrap/réallocations par frame.
  - Invalidation massive aux bons endroits : resize/changement thème-langue-debug (clé),
    changement complet de session et reset chat (`hard_reset()`).
  - Filet de sécurité : goldens du compteur de wrap vs rendu réel ratatui (6 largeurs × 5
    fixtures ASCII/CJK/emoji/spans stylés) + découverte documentée : `Line::from(String)`
    normalise les `\n` internes en spans séparés dès la construction.
  - Suite portée à **92 tests verts**, clippy `-D warnings` clean.

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
