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
  `⟳ 👻 💭 Réflexion profonde…` / `⟳ 👻 💭 Deep thinking…` parcourue d'un **shimmer
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
