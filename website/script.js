/**
 * 🧞 SPIRITTY — INTERACTIVE LOGIC & I18N
 * Features: Instant FR/EN switch, One-Click Copy, Interactive TUI Scenarios
 */

// ==============================================================================
// 1. I18N DICTIONARY (FR / EN)
// ==============================================================================
const translations = {
    fr: {
        nav_features: "Fonctionnalités",
        nav_demo: "Démonstration",
        nav_install: "Installation",
        nav_shortcuts: "Raccourcis",
        hero_badge: "✨ Version 0.7.3 disponible — Binaire TUI Rust natif",
        hero_title: "L'assistant IA pour terminal <span class=\"gradient-text\">nouvelle génération</span>",
        hero_subtitle: "Conçu par un sysadmin pour les sysadmins, ingénieurs DevOps et power users. Split-screen ergonomique : agent IA contextuel à gauche, shell PTY natif 100% interactif à droite.",
        install_tab_official: "Officiel (spiritty.ai)",
        install_tab_github: "Miroir GitHub (Raw)",
        copy_btn: "Copier",
        copied_btn: "Copié !",
        toast_copied: "Commande copiée dans le presse-papiers !",
        platforms_label: "Compatible Linux (x86_64, aarch64) & macOS Apple Silicon (aarch64)",
        pill_rust: "<strong>100% Rust</strong> & Ultra-rapide",
        pill_pty: "<strong>PTY Natif</strong> & Zéro pollution",
        pill_hitl: "<strong>Human-in-the-Loop</strong> Strict",
        pill_models: "<strong>Multi-Providers</strong> (Cloud & Local)",
        
        demo_tag: "DÉMONSTRATION INTERACTIVE",
        demo_title: "L'expérience Spiritty en direct",
        demo_desc: "Explorez comment Spiritty collabore en temps réel dans votre terminal sans jamais interrompre votre frappe ni polluer votre historique shell.",
        scenario_perf: "🚀 Optimisation Serveur",
        scenario_ssh: "🌐 Contexte SSH & Détection",
        scenario_diag: "🛠️ Diagnostic Proactif (Alt+D)",
        scenario_mcp: "🔌 Outils & Protocole MCP",

        feat_tag: "CONÇU POUR L'ADMINISTRATION SYSTÈME",
        feat_title: "Pourquoi Spiritty surpasse les outils existants",
        feat_desc: "Chaque détail a été pensé pour respecter l'ergonomie sacrée du terminal Linux et accélérer les opérations de production.",
        
        feat_1_title: "Split-Screen & Zéro Pollution",
        feat_1_desc: "Votre shell à droite reste 100% interactif en continu (bash, zsh, fish). Aucune commande n'est insérée en douce ni cachée avec des sentinelles visibles dans votre PTY.",
        feat_2_title: "Human-in-the-Loop & Sécurité",
        feat_2_desc: "Aucune commande ne tourne sans votre accord explicite. Classification stricte des risques (Safe, Sudo, Risky) et modes d'approbation ajustables en une touche (F3).",
        feat_3_title: "Contexte Hôte & Détection SSH",
        feat_3_desc: "Surveillance temps réel sous le PTY : Spiritty détecte vos connexions SSH, profile la distribution distante (Debian, Ubuntu, Arch...) et bascule instantanément son prompt.",
        feat_4_title: "Multi-Providers & LLMs Locaux",
        feat_4_desc: "DeepSeek V4.1 Flash, Claude 3.7 Sonnet, Gemini 2.5, Grok, OpenAI, mais aussi vos serveurs locaux Ollama et LM Studio pour une confidentialité totale.",
        feat_5_title: "Support Protocole MCP",
        feat_5_desc: "Intégrez vos serveurs MCP (Model Context Protocol) en JSON-RPC pour connecter Docker, Kubernetes, bases de données et outils custom via la modale Ctrl+M.",
        feat_6_title: "Métriques Réelles & Coûts Live",
        feat_6_desc: "Suivi exact du débit (tokens/s réels), décompte précis des fenêtres de contexte et estimation en direct du coût de votre session au centième de centime.",

        install_tag: "DÉMARRAGE EN 30 SECONDES",
        install_title: "Installez Spiritty facilement",
        install_desc: "Téléchargez le binaire précompilé officiel ou compilez depuis les sources Cargo en quelques commandes.",
        inst_opt1_title: "1. Installateur One-Line (Recommandé)",
        inst_opt1_desc: "Détecte automatiquement votre architecture, télécharge la dernière version, installe le binaire dans ~/.local/bin et configure le lanceur bureau avec icône.",
        inst_opt2_title: "2. Compilation depuis les Sources",
        inst_opt2_desc: "Vous avez Rust installé ? Clonez le dépôt et construisez le binaire autonome optimisé en une minute chrono.",
        inst_opt3_title: "3. Archives Binaires Directes",
        inst_opt3_desc: "Téléchargez directement les archives tarball légères pour Linux x86_64, aarch64 ou macOS Apple Silicon depuis GitHub Releases.",
        btn_view_releases: "Voir les releases GitHub →",

        shortcuts_tag: "ERGONOMIE CLAVIER",
        shortcuts_title: "Raccourcis essentiels",
        shortcuts_desc: "Restez au clavier : tout est accessible en une combinaison de touches intuitive.",
        col_key: "Raccourci",
        col_action: "Action",
        col_context: "Utilité",
        k1_action: "Bascule de focus",
        k1_desc: "Passe instantanément du panneau Chat au shell Terminal natif",
        k2_action: "Exécuter proposition N",
        k2_desc: "Injecte et exécute la commande #N suggérée par Spiritty",
        k3_action: "Diagnostic proactif",
        k3_desc: "Analyse immédiatement la dernière commande shell échouée dans le PTY",
        k4_action: "Configuration & Modèles",
        k4_desc: "Changez de provider, de modèle ou de clé API à la volée",
        k5_action: "Niveau d'approbation",
        k5_desc: "Basculez entre Safe, Sudo, Yolo et Strict",
        k6_action: "Gestionnaire MCP",
        k6_desc: "Inspectez, activez ou configurez vos serveurs et outils MCP",
        k7_action: "Historique des sessions",
        k7_desc: "Naviguez, restaurez ou compactez vos conversations archivées",

        footer_desc: "Spiritty est un projet Open Source créé pour libérer le potentiel des administrateurs système et ingénieurs d'infrastructure.",
        footer_links_col1: "Ressources",
        footer_links_col2: "Projet",
        footer_copyright: "© 2026 Spiritty Contributors. Distribué sous licences MIT ou Apache-2.0."
    },
    en: {
        nav_features: "Features",
        nav_demo: "Demo",
        nav_install: "Install",
        nav_shortcuts: "Shortcuts",
        hero_badge: "✨ Version 0.7.3 available — Standalone Rust TUI binary",
        hero_title: "Next-generation AI terminal <span class=\"gradient-text\">split-screen companion</span>",
        hero_subtitle: "Built by a sysadmin for sysadmins, DevOps engineers, and power users. Ergonomic split-screen: proactive AI assistant on the left, fully interactive native PTY shell on the right.",
        install_tab_official: "Official (spiritty.ai)",
        install_tab_github: "GitHub Mirror (Raw)",
        copy_btn: "Copy",
        copied_btn: "Copied!",
        toast_copied: "Command copied to clipboard!",
        platforms_label: "Supports Linux (x86_64, aarch64) & macOS Apple Silicon (aarch64)",
        pill_rust: "<strong>100% Rust</strong> & Ultra-fast",
        pill_pty: "<strong>Native PTY</strong> & Zero pollution",
        pill_hitl: "Strict <strong>Human-in-the-Loop</strong>",
        pill_models: "<strong>Multi-Providers</strong> (Cloud & Local)",

        demo_tag: "INTERACTIVE SHOWCASE",
        demo_title: "The Spiritty Experience Live",
        demo_desc: "See how Spiritty collaborates in real time inside your terminal without interrupting keystrokes or polluting your shell history.",
        scenario_perf: "🚀 Server Optimization",
        scenario_ssh: "🌐 SSH Context & Detection",
        scenario_diag: "🛠️ Proactive Diagnosis (Alt+D)",
        scenario_mcp: "🔌 MCP Tools Protocol",

        feat_tag: "BUILT FOR PRODUCTION SYSADMINS",
        feat_title: "Why Spiritty Outperforms Existing Tools",
        feat_desc: "Engineered specifically to respect the sanctity of your terminal while accelerating mission-critical server management.",

        feat_1_title: "Split-Screen & Zero Pollution",
        feat_1_desc: "Your right-hand shell remains 100% interactive at all times (bash, zsh, fish). No injected dummy sentinels, no hidden commands in your history.",
        feat_2_title: "Human-in-the-Loop & Safety",
        feat_2_desc: "No command executes without your explicit consent. Granular risk classification (Safe, Sudo, Risky) with one-key approval toggling (F3).",
        feat_3_title: "Host Context & SSH Awareness",
        feat_3_desc: "Live non-blocking /proc monitoring: Spiritty detects SSH hops, profiles the remote distro (Debian, Ubuntu, Arch...), and switches system prompts instantly.",
        feat_4_title: "Multi-Providers & Local LLMs",
        feat_4_desc: "DeepSeek V4.1 Flash, Claude 3.7 Sonnet, Gemini 2.5, Grok, OpenAI, plus your local Ollama and LM Studio models for complete offline privacy.",
        feat_5_title: "Model Context Protocol (MCP)",
        feat_5_desc: "Connect your MCP servers via JSON-RPC to inspect and invoke Docker, Kubernetes, database, or custom tooling via the interactive Ctrl+M modal.",
        feat_6_title: "Precise Metrics & Live Costs",
        feat_6_desc: "True token throughput (tokens/s), real-time context budget tracking, and live session cost estimations down to a fraction of a cent.",

        install_tag: "GET STARTED IN 30 SECONDS",
        install_title: "Install Spiritty Effortlessly",
        install_desc: "Download the prebuilt official binary or build from source via Cargo in seconds.",
        inst_opt1_title: "1. One-Line Installer (Recommended)",
        inst_opt1_desc: "Automatically detects your system architecture, downloads the release, installs into ~/.local/bin, and sets up your desktop launcher with icons.",
        inst_opt2_title: "2. Build from Source",
        inst_opt2_desc: "Have Rust installed? Clone the repository and compile the optimized standalone binary in less than a minute.",
        inst_opt3_title: "3. Direct Release Tarballs",
        inst_opt3_desc: "Download lightweight tarball archives directly for Linux x86_64, aarch64, or macOS Apple Silicon from GitHub Releases.",
        btn_view_releases: "View GitHub Releases →",

        shortcuts_tag: "KEYBOARD ERGONOMICS",
        shortcuts_title: "Essential Shortcuts",
        shortcuts_desc: "Stay entirely on your keyboard: all core actions are mapped to intuitive shortcuts.",
        col_key: "Shortcut",
        col_action: "Action",
        col_context: "Purpose",
        k1_action: "Focus Switch",
        k1_desc: "Instantly toggles focus between AI Chat and native Terminal shell",
        k2_action: "Execute Proposal N",
        k2_desc: "Injects and executes the suggested command #N in the right panel",
        k3_action: "Proactive Diagnosis",
        k3_desc: "Instantly analyzes the last failed shell command in the PTY",
        k4_action: "Config & Models",
        k4_desc: "Switch provider, model, reasoning effort or API key on the fly",
        k5_action: "Approval Mode",
        k5_desc: "Cycle approval levels between Safe, Sudo, Yolo, and Strict",
        k6_action: "MCP Manager",
        k6_desc: "Inspect, toggle, and configure your MCP servers and tools",
        k7_action: "Session History",
        k7_desc: "Browse, reload, search, or manually compact past sessions",

        footer_desc: "Spiritty is an Open Source project created to empower sysadmins, DevOps, and infrastructure engineers.",
        footer_links_col1: "Resources",
        footer_links_col2: "Project",
        footer_copyright: "© 2026 Spiritty Contributors. Licensed under MIT or Apache-2.0."
    }
};

// ==============================================================================
// ==============================================================================
// 2. SCENARIO DATA FOR AUTHENTIC SPIRITTY RATATUI DEMO
// ==============================================================================
const demoScenarios = {
    perf: {
        left_lines: "81 l.",
        right_host: "🌐 SSH bento.moondogs.fr (reprise)",
        right_lines: "58 l.",
        footer_model: "gemini-3.8-flash",
        footer_reasoning: "Med",
        footer_tok: "⚡ 2.4k tok",
        footer_cost: "💵 $0.0221",
        footer_ctx: "📖 Ctx: 1.6k / 1.0M (0%)",
        footer_approve: "F3 Sudo",
        chat_flow: {
            fr: `
<div class="chat-lead-error">connect() to unix:/var/run/php/php7.3-fpm.sock failed (2: No such file or directory)</div>
<div class="chat-text">Nginx essaie d'utiliser PHP 7.3 (/var/run/php/php7.3-fpm.sock) dans /etc/nginx/sites-available/default, mais seules les versions PHP 5.6 et PHP 7.4 tournent actuellement sur le serveur.</div>
<div class="chat-text">Je vérifie la configuration exacte du bloc phpMyAdmin dans /etc/nginx/sites-available/default :</div>
<div class="tui-tool-line">
    <span class="tui-tool-icon">💻</span>
    <span class="tui-tool-cmd">sed -n '50,75p' /etc/nginx/sites-available/default</span>
</div>
<div class="tui-agent-remark">
    <span class="tui-agent-icon">🧞</span>
    <span>Je vérifie l'état du service php7.3-fpm :</span>
</div>
<div class="tui-tool-line">
    <span class="tui-tool-icon">💻</span>
    <span class="tui-tool-cmd">systemctl status php7.3-fpm</span>
</div>
<div class="tui-agent-remark">
    <span class="tui-agent-icon">🧞</span>
    <span>Le service php7.3-fpm est installé mais désactivé / arrêté (<span style="color:#f59e0b">inactive (dead)</span>), ce qui empêche Nginx de contacter le socket PHP pour servir phpMyAdmin.</span>
</div>
<div class="chat-text">Vous avez deux options pour résoudre le problème :</div>
<div class="chat-text">Option 1 : Démarrer et activer <span class="chat-highlight">php7.3-fpm</span> (recommandé si configuré ainsi à l'origine)</div>

<div class="tui-cmd-card">
    <div class="tui-cmd-header">
        <span>⚡ COMMANDE #1</span>
        <span class="cmd-badge sudo">Sudo</span>
    </div>
    <div class="tui-cmd-box">sudo systemctl enable --now php7.3-fpm</div>
    <div class="tui-cmd-footer">
        <span>Validation requise avant exécution</span>
        <div class="cmd-key-combo">
            <span class="cmd-pill-btn">Alt + 1</span>
        </div>
    </div>
</div>

<div class="chat-text">Option 2 : Basculer phpMyAdmin sur <span class="chat-highlight">php7.4-fpm</span> (déjà actif sur le serveur)</div>

<div class="tui-cmd-card">
    <div class="tui-cmd-header">
        <span>⚡ COMMANDE #2</span>
        <span class="cmd-badge risky">Risqué</span>
    </div>
    <div class="tui-cmd-box">sudo sed -i 's/php7.3-fpm.sock/php7.4-fpm.sock/' /etc/nginx/sites-available/default &amp;&amp; sudo nginx -t &amp;&amp; sudo systemctl reload nginx</div>
    <div class="tui-cmd-footer">
        <span>Validation requise avant exécution</span>
        <div class="cmd-key-combo">
            <span class="cmd-pill-btn">Alt + 2</span>
        </div>
    </div>
</div>

<div class="tui-tool-line">
    <span class="tui-tool-icon">💻</span>
    <span class="tui-tool-cmd">sudo sed -i 's/php7.3-fpm.sock/php7.4-fpm.sock/' /etc/nginx/sites-available/default &amp;&amp; sudo nginx -t &amp;&amp; sudo systemctl reload nginx</span>
</div>
<div class="tui-agent-remark">
    <span class="tui-agent-icon">🧞</span>
    <span>La configuration Nginx a été mise à jour vers PHP 7.4 et rechargée avec succès.</span>
</div>
<div class="chat-text">Je vérifie que phpMyAdmin répond désormais correctement (sans erreur 502) :</div>
<div class="tui-tool-line">
    <span class="tui-tool-icon">💻</span>
    <span class="tui-tool-cmd">curl -sI http://localhost/phpmyadmin/ | head -n 5</span>
</div>
<div class="tui-agent-remark">
    <span class="tui-agent-icon">🧞</span>
    <span>Le problème est résolu.</span>
</div>
<div class="chat-text" style="margin-top: 4px;">
    <strong>Résumé de l'intervention :</strong><br>
    • Cause de l'erreur 502 : Nginx cherchait à joindre le socket <span class="chat-highlight">/var/run/php/php7.3-fpm.sock</span>, mais le service php7.3-fpm était arrêté et désactivé.<br>
    • Action effectuée : Mise à jour du vhost Nginx par défaut pour router les requêtes PHP vers <span class="chat-highlight">/var/run/php/php7.4-fpm.sock</span> (service déjà actif sur la machine) et rechargement de Nginx.<br>
    • Validation : Le serveur renvoie désormais un code '<span class="color-green">HTTP 200 OK</span>' sur /phpmyadmin/.<br><br>
    L'accès à votre interface phpMyAdmin est de nouveau opérationnel depuis votre navigateur.
</div>
`,
            en: `
<div class="chat-lead-error">connect() to unix:/var/run/php/php7.3-fpm.sock failed (2: No such file or directory)</div>
<div class="chat-text">Nginx is attempting to use PHP 7.3 (/var/run/php/php7.3-fpm.sock) in /etc/nginx/sites-available/default, but only PHP 5.6 and PHP 7.4 are currently active on this server.</div>
<div class="chat-text">Checking the exact phpMyAdmin configuration block in /etc/nginx/sites-available/default :</div>
<div class="tui-tool-line">
    <span class="tui-tool-icon">💻</span>
    <span class="tui-tool-cmd">sed -n '50,75p' /etc/nginx/sites-available/default</span>
</div>
<div class="tui-agent-remark">
    <span class="tui-agent-icon">🧞</span>
    <span>Checking status of the php7.3-fpm service :</span>
</div>
<div class="tui-tool-line">
    <span class="tui-tool-icon">💻</span>
    <span class="tui-tool-cmd">systemctl status php7.3-fpm</span>
</div>
<div class="tui-agent-remark">
    <span class="tui-agent-icon">🧞</span>
    <span>The php7.3-fpm service is installed but disabled / stopped (<span style="color:#f59e0b">inactive (dead)</span>), preventing Nginx from contacting the PHP socket for phpMyAdmin.</span>
</div>
<div class="chat-text">You have two options to resolve this issue :</div>
<div class="chat-text">Option 1 : Start and enable <span class="chat-highlight">php7.3-fpm</span> (recommended if originally provisioned)</div>

<div class="tui-cmd-card">
    <div class="tui-cmd-header">
        <span>⚡ COMMANDE #1</span>
        <span class="cmd-badge sudo">Sudo</span>
    </div>
    <div class="tui-cmd-box">sudo systemctl enable --now php7.3-fpm</div>
    <div class="tui-cmd-footer">
        <span>Confirmation required before execution</span>
        <div class="cmd-key-combo">
            <span class="cmd-pill-btn">Alt + 1</span>
        </div>
    </div>
</div>

<div class="chat-text">Option 2 : Switch phpMyAdmin to <span class="chat-highlight">php7.4-fpm</span> (already active on the server)</div>

<div class="tui-cmd-card">
    <div class="tui-cmd-header">
        <span>⚡ COMMANDE #2</span>
        <span class="cmd-badge risky">Risky</span>
    </div>
    <div class="tui-cmd-box">sudo sed -i 's/php7.3-fpm.sock/php7.4-fpm.sock/' /etc/nginx/sites-available/default &amp;&amp; sudo nginx -t &amp;&amp; sudo systemctl reload nginx</div>
    <div class="tui-cmd-footer">
        <span>Confirmation required before execution</span>
        <div class="cmd-key-combo">
            <span class="cmd-pill-btn">Alt + 2</span>
        </div>
    </div>
</div>

<div class="tui-tool-line">
    <span class="tui-tool-icon">💻</span>
    <span class="tui-tool-cmd">sudo sed -i 's/php7.3-fpm.sock/php7.4-fpm.sock/' /etc/nginx/sites-available/default &amp;&amp; sudo nginx -t &amp;&amp; sudo systemctl reload nginx</span>
</div>
<div class="tui-agent-remark">
    <span class="tui-agent-icon">🧞</span>
    <span>Nginx configuration successfully updated to PHP 7.4 and reloaded.</span>
</div>
<div class="chat-text">Verifying phpMyAdmin responds correctly (without error 502) :</div>
<div class="tui-tool-line">
    <span class="tui-tool-icon">💻</span>
    <span class="tui-tool-cmd">curl -sI http://localhost/phpmyadmin/ | head -n 5</span>
</div>
<div class="tui-agent-remark">
    <span class="tui-agent-icon">🧞</span>
    <span>The issue is resolved.</span>
</div>
<div class="chat-text" style="margin-top: 4px;">
    <strong>Intervention Summary :</strong><br>
    • Root Cause of 502 : Nginx sought socket <span class="chat-highlight">/var/run/php/php7.3-fpm.sock</span>, but php7.3-fpm was disabled and stopped.<br>
    • Action taken : Updated default Nginx vhost to route PHP requests to <span class="chat-highlight">/var/run/php/php7.4-fpm.sock</span> (active service) and reloaded Nginx.<br>
    • Validation : Server now returns '<span class="color-green">HTTP 200 OK</span>' on /phpmyadmin/.<br><br>
    Access to your phpMyAdmin interface is operational again from your browser.
</div>
`
        },
        term_content: `
<div class="term-line"><span class="term-preprompt">~}</span> <span class="term-user-cmd">l</span></div>
<div class="term-line color-dim">drwxr-xr-x  - xorne 10 sept. 18:27 -I .git</div>
<div class="term-line color-dim">drwxr-xr-x  - xorne 30 août  13:19 -h .github</div>
<div class="term-line">.rw-r--r-- 218 xorne 30 août  13:19 -- .gitignore</div>
<div class="term-line">.rw-r--r-- 5,6k xorne 30 août  13:31 -- <span class="color-green">AGENTS.md</span></div>
<div class="term-line">.rw-r--r-- 8,9k xorne 30 août  13:19 -- ARCHITECTURE.md</div>
<div class="term-line"><span class="color-blue">drwxr-xr-x  - xorne 10 sept. 17:56 -- assets</span></div>
<div class="term-line">.rw-r--r-- 76k xorne 10 sept. 14:52 -- Cargo.lock</div>
<div class="term-line">.rw-r--r-- 1,2k xorne 10 sept. 14:52 -- <span class="color-yellow">Cargo.toml</span></div>
<div class="term-line">.rw-r--r-- 80k xorne 10 sept. 17:50 -- CHANGELOG.md</div>
<div class="term-line">.rw-r--r-- 1,5k xorne  3 sept. 12:28 -- CONTEXT.md</div>
<div class="term-line"><span class="color-blue">drwxr-xr-x  - xorne  3 sept. 12:28 -- docs</span></div>
<div class="term-line">.rwxr-xr-x 13k xorne 10 sept. 18:07 -- <span class="color-green">install.sh</span></div>
<div class="term-line">.rw-r--r-- 1,1k xorne 30 août  13:19 -- LICENSE</div>
<div class="term-line">.rw-r--r-- 9,7k xorne 10 sept. 17:57 -- <span class="color-yellow">README_fr.md</span></div>
<div class="term-line">.rw-r--r-- 8,8k xorne 10 sept. 17:57 -- <span class="color-yellow">README.md</span></div>
<div class="term-line">.rw-r--r-- 17k xorne 10 sept. 14:53 -- ROADMAP.md</div>
<div class="term-line"><span class="color-blue">drwxr-xr-x  - xorne 30 août  16:50 -- src</span></div>
<div class="term-line color-dim">drwxr-xr-x  - xorne  3 sept. 12:43 -I target</div>
<div class="term-line"><span class="color-blue">drwxr-xr-x  - xorne 30 août  15:24 -- tests</span></div>
<div class="term-line"><span class="color-blue">drwxr-xr-x  - xorne 10 sept. 18:16 <span class="color-dim">-N</span> website</span></div>
<div class="prompt-line">
    <div class="prompt-left">
        <span class="term-path">~/Projets/Spiritty</span> <span class="term-branch">main*</span> <span class="term-prompt-arrow">&gt;</span> <span class="term-cursor"></span>
    </div>
    <div class="prompt-time">18:27:21</div>
</div>
`
    },
    ssh: {
        left_lines: "42 l.",
        right_host: "🌐 SSH root@192.168.1.50 (Debian 12)",
        right_lines: "36 l.",
        footer_model: "deepseek-chat",
        footer_reasoning: "Off",
        footer_tok: "⚡ 114 tok",
        footer_cost: "💵 $0.0018",
        footer_ctx: "📖 Ctx: 2.1k / 128k (2%)",
        footer_approve: "F3 Safe",
        chat_flow: {
            fr: `
<div class="chat-text"><strong>xorne ❯</strong> vérifie la santé de la machine distante et les mises à jour de sécurité</div>
<div class="think-block">
    💭 Réflexion : Détection de la session SSH root@192.168.1.50. Système Debian GNU/Linux 12 (Bookworm). Adaptation des commandes apt-get et métriques /proc...
</div>
<div class="tui-agent-remark">
    <span class="tui-agent-icon">🧞</span>
    <span>Connecté à Debian 12 distant. Voici la commande sécurisée recommandée pour auditer les paquets de sécurité et la charge :</span>
</div>

<div class="tui-cmd-card">
    <div class="tui-cmd-header">
        <span>⚡ COMMANDE #1</span>
        <span class="cmd-badge safe">Safe</span>
    </div>
    <div class="tui-cmd-box">apt-get -s upgrade | grep -i security ; uptime ; free -h</div>
    <div class="tui-cmd-footer">
        <span>Validation requise avant exécution</span>
        <div class="cmd-key-combo">
            <span class="cmd-pill-btn">Alt + 1</span>
        </div>
    </div>
</div>

<div class="tui-tool-line">
    <span class="tui-tool-icon">💻</span>
    <span class="tui-tool-cmd">apt-get -s upgrade | grep -i security ; uptime ; free -h</span>
</div>
<div class="tui-agent-remark">
    <span class="tui-agent-icon">🧞</span>
    <span>Rapport de santé distant :<br>
    • 1 mise à jour critique en attente : <span class="chat-highlight">linux-image-6.1.0-22-amd64</span> (Debian 12 stable-security).<br>
    • Uptime : 42 jours sans interruption, charge minimale (0.12).<br>
    • Mémoire : 4.2 GiB utilisés sur 31 GiB disponibles (26 GiB disponibles).
    </span>
</div>
`,
            en: `
<div class="chat-text"><strong>xorne ❯</strong> check remote host health and critical security updates</div>
<div class="think-block">
    💭 Thinking : Remote SSH session detected on root@192.168.1.50. Target distro is Debian GNU/Linux 12 (Bookworm). Adapting apt-get queries and /proc metrics...
</div>
<div class="tui-agent-remark">
    <span class="tui-agent-icon">🧞</span>
    <span>Connected to remote Debian 12. Recommended safe command to audit security updates and resource consumption :</span>
</div>

<div class="tui-cmd-card">
    <div class="tui-cmd-header">
        <span>⚡ COMMANDE #1</span>
        <span class="cmd-badge safe">Safe</span>
    </div>
    <div class="tui-cmd-box">apt-get -s upgrade | grep -i security ; uptime ; free -h</div>
    <div class="tui-cmd-footer">
        <span>Confirmation required before execution</span>
        <div class="cmd-key-combo">
            <span class="cmd-pill-btn">Alt + 1</span>
        </div>
    </div>
</div>

<div class="tui-tool-line">
    <span class="tui-tool-icon">💻</span>
    <span class="tui-tool-cmd">apt-get -s upgrade | grep -i security ; uptime ; free -h</span>
</div>
<div class="tui-agent-remark">
    <span class="tui-agent-icon">🧞</span>
    <span>Remote health assessment :<br>
    • 1 critical security patch pending : <span class="chat-highlight">linux-image-6.1.0-22-amd64</span> (Debian 12 stable-security).<br>
    • Uptime : 42 days uninterrupted, lightweight load average (0.12).<br>
    • Memory : 4.2 GiB used out of 31 GiB (26 GiB available).
    </span>
</div>
`
        },
        term_content: `
<div class="term-line"><span class="term-preprompt">root@deb12-srv01:~#</span> <span class="term-user-cmd">apt-get -s upgrade | grep -i security ; uptime ; free -h</span></div>
<div class="term-line color-green">Inst linux-image-6.1.0-22-amd64 [6.1.94-1] (6.1.99-1 Debian:12/stable-security [amd64])</div>
<div class="term-line">18:12:04 up 42 days,  3:14,  1 user,  load average: 0.12, 0.08, 0.05</div>
<div class="term-line color-dim">               total        used        free      shared  buff/cache   available</div>
<div class="term-line">Mem:            31Gi       4.2Gi        21Gi       128Mi       5.8Gi        26Gi</div>
<div class="term-line">Swap:          4.0Gi          0B       4.0Gi</div>
<div class="prompt-line">
    <div class="prompt-left">
        <span class="term-path">root@deb12-srv01</span>:<span class="term-branch">~#</span> <span class="term-cursor"></span>
    </div>
    <div class="prompt-time">18:28:44</div>
</div>
`
    },
    diag: {
        left_lines: "35 l.",
        right_host: "local: xorne@cachyos-desktop",
        right_lines: "24 l.",
        footer_model: "claude-3-7-sonnet",
        footer_reasoning: "Med",
        footer_tok: "⚡ 82 tok",
        footer_cost: "💵 $0.0084",
        footer_ctx: "📖 Ctx: 3.4k / 200k (1%)",
        footer_approve: "F3 Safe",
        chat_flow: {
            fr: `
<div class="chat-lead-error">[Détection automatique Alt+D] Échec de la commande 'systemctl start apache2' (Code 1)</div>
<div class="think-block">
    💭 Réflexion : Capture automatique de la commande échouée dans le PTY. Détection d'un conflit de socket HTTP sur le port standard...
</div>
<div class="tui-agent-remark">
    <span class="tui-agent-icon">🧞</span>
    <span>Le port 80/tcp est déjà occupé par Nginx. Apache ne parvient pas à lier son écoute. Je propose d'identifier le processus concurrent :</span>
</div>

<div class="tui-cmd-card">
    <div class="tui-cmd-header">
        <span>⚡ COMMANDE #1</span>
        <span class="cmd-badge safe">Safe</span>
    </div>
    <div class="tui-cmd-box">sudo ss -tulpn | grep -E ':(80|443)'</div>
    <div class="tui-cmd-footer">
        <span>Validation requise avant exécution</span>
        <div class="cmd-key-combo">
            <span class="cmd-pill-btn">Alt + 1</span>
        </div>
    </div>
</div>

<div class="tui-tool-line">
    <span class="tui-tool-icon">💻</span>
    <span class="tui-tool-cmd">sudo ss -tulpn | grep -E ':(80|443)'</span>
</div>
<div class="tui-agent-remark">
    <span class="tui-agent-icon">🧞</span>
    <span>Port 80 écouté par pid=842 (nginx). Solutions : arrêter Nginx via <span class="chat-highlight">sudo systemctl stop nginx</span> ou réassigner Apache sur le port 8080 dans /etc/apache2/ports.conf.</span>
</div>
`,
            en: `
<div class="chat-lead-error">[Automatic Alt+D Capture] Failed command 'systemctl start apache2' (Exit code 1)</div>
<div class="think-block">
    💭 Thinking : Captured failed shell invocation from right PTY. Analyzing HTTP socket port collision...
</div>
<div class="tui-agent-remark">
    <span class="tui-agent-icon">🧞</span>
    <span>Port 80/tcp is already bound by Nginx. Apache cannot bind its listener socket. Proposed safe command to inspect listeners :</span>
</div>

<div class="tui-cmd-card">
    <div class="tui-cmd-header">
        <span>⚡ COMMANDE #1</span>
        <span class="cmd-badge safe">Safe</span>
    </div>
    <div class="tui-cmd-box">sudo ss -tulpn | grep -E ':(80|443)'</div>
    <div class="tui-cmd-footer">
        <span>Confirmation required before execution</span>
        <div class="cmd-key-combo">
            <span class="cmd-pill-btn">Alt + 1</span>
        </div>
    </div>
</div>

<div class="tui-tool-line">
    <span class="tui-tool-icon">💻</span>
    <span class="tui-tool-cmd">sudo ss -tulpn | grep -E ':(80|443)'</span>
</div>
<div class="tui-agent-remark">
    <span class="tui-agent-icon">🧞</span>
    <span>Port 80 occupied by pid=842 (nginx). Remedies : stop Nginx via <span class="chat-highlight">sudo systemctl stop nginx</span> or rebind Apache to port 8080 in /etc/apache2/ports.conf.</span>
</div>
`
        },
        term_content: `
<div class="term-line"><span class="term-preprompt">xorne@cachyos-desktop:~/Projets/Spiritty$</span> <span class="term-user-cmd">sudo systemctl start apache2</span></div>
<div class="term-line color-dim">Job for apache2.service failed because the control process exited with error code.</div>
<div class="term-line color-dim">See "systemctl status apache2.service" and "journalctl -xeu apache2.service" for details.</div>
<div class="term-line color-yellow">[Spiritty Toast: ⚠️ Commande échouée (1) — Alt+D Diagnostiquer]</div>
<div class="prompt-line">
    <div class="prompt-left">
        <span class="term-path">~/Projets/Spiritty</span> <span class="term-branch">main*</span> <span class="term-prompt-arrow">&gt;</span> <span class="term-cursor"></span>
    </div>
    <div class="prompt-time">18:29:10</div>
</div>
`
    },
    mcp: {
        left_lines: "48 l.",
        right_host: "local: xorne@cachyos-desktop",
        right_lines: "30 l.",
        footer_model: "gemini-3.8-flash",
        footer_reasoning: "Off",
        footer_tok: "⚡ 142 tok",
        footer_cost: "💵 $0.0032",
        footer_ctx: "📖 Ctx: 4.8k / 1.0M (0%)",
        footer_approve: "F3 Sudo",
        chat_flow: {
            fr: `
<div class="chat-text"><strong>xorne ❯</strong> liste l'état et la consommation des conteneurs via le serveur MCP Docker</div>
<div class="think-block">
    💭 Réflexion : Appel de l'outil MCP 'docker:stats' via le protocole standardisé JSON-RPC configuré dans la modale Ctrl+M...
</div>
<div class="tui-agent-remark">
    <span class="tui-agent-icon">🧞</span>
    <span>Le serveur MCP Docker a validé l'appel. Proposition pour relever les métriques d'exécution en temps réel :</span>
</div>

<div class="tui-cmd-card">
    <div class="tui-cmd-header">
        <span>⚡ OUTIL MCP : docker:stats</span>
        <span class="cmd-badge safe">Standard</span>
    </div>
    <div class="tui-cmd-box">docker stats --no-stream --format "table {{.Name}}\t{{.CPUPerc}}\t{{.MemUsage}}"</div>
    <div class="tui-cmd-footer">
        <span>Appel via protocole JSON-RPC standardisé</span>
        <div class="cmd-key-combo">
            <span class="cmd-pill-btn">Alt + 1</span>
        </div>
    </div>
</div>

<div class="tui-tool-line">
    <span class="tui-tool-icon">💻</span>
    <span class="tui-tool-cmd">docker stats --no-stream --format "table {{.Name}}\t{{.CPUPerc}}\t{{.MemUsage}}"</span>
</div>
<div class="tui-agent-remark">
    <span class="tui-agent-icon">🧞</span>
    <span>Métriques des conteneurs de production :<br>
    • <span class="chat-highlight">prod-postgres</span> : CPU 1.2% | RAM 412MiB / 31Gi (healthy)<br>
    • <span class="chat-highlight">prod-redis</span> : CPU 0.1% | RAM 38MiB / 31Gi (healthy)<br>
    Services stables, aucune surcharge.
    </span>
</div>
`,
            en: `
<div class="chat-text"><strong>xorne ❯</strong> list container health and metrics via the Docker MCP server</div>
<div class="think-block">
    💭 Thinking : Invoking MCP tool 'docker:stats' over standard JSON-RPC configured in Ctrl+M modal...
</div>
<div class="tui-agent-remark">
    <span class="tui-agent-icon">🧞</span>
    <span>The Docker MCP server validated the call. Proposal to inspect live container resource footprints :</span>
</div>

<div class="tui-cmd-card">
    <div class="tui-cmd-header">
        <span>⚡ OUTIL MCP : docker:stats</span>
        <span class="cmd-badge safe">Standard</span>
    </div>
    <div class="tui-cmd-box">docker stats --no-stream --format "table {{.Name}}\t{{.CPUPerc}}\t{{.MemUsage}}"</div>
    <div class="tui-cmd-footer">
        <span>Standard JSON-RPC protocol execution</span>
        <div class="cmd-key-combo">
            <span class="cmd-pill-btn">Alt + 1</span>
        </div>
    </div>
</div>

<div class="tui-tool-line">
    <span class="tui-tool-icon">💻</span>
    <span class="tui-tool-cmd">docker stats --no-stream --format "table {{.Name}}\t{{.CPUPerc}}\t{{.MemUsage}}"</span>
</div>
<div class="tui-agent-remark">
    <span class="tui-agent-icon">🧞</span>
    <span>Production container metrics :<br>
    • <span class="chat-highlight">prod-postgres</span> : CPU 1.2% | RAM 412MiB / 31Gi (healthy)<br>
    • <span class="chat-highlight">prod-redis</span> : CPU 0.1% | RAM 38MiB / 31Gi (healthy)<br>
    Services operating smoothly without contention.
    </span>
</div>
`
        },
        term_content: `
<div class="term-line"><span class="term-preprompt">xorne@cachyos-desktop:~/Projets/Spiritty$</span> <span class="term-user-cmd">docker ps --format 'table {{.Names}}\t{{.Status}}\t{{.Ports}}'</span></div>
<div class="term-line color-dim">NAMES           STATUS                  PORTS</div>
<div class="term-line">prod-postgres   Up 14 days (healthy)    0.0.0.0:5432-&gt;5432/tcp</div>
<div class="term-line">prod-redis      Up 14 days (healthy)    0.0.0.0:6379-&gt;6379/tcp</div>
<div class="prompt-line">
    <div class="prompt-left">
        <span class="term-path">~/Projets/Spiritty</span> <span class="term-branch">main*</span> <span class="term-prompt-arrow">&gt;</span> <span class="term-cursor"></span>
    </div>
    <div class="prompt-time">18:30:05</div>
</div>
`
    }
};

let activeScenario = "perf";

// ==============================================================================
// 3. INITIALIZATION & EVENT HANDLERS
// ==============================================================================
document.addEventListener("DOMContentLoaded", () => {
    // 3.1 Language Detection & Setup
    let currentLang = localStorage.getItem("spiritty_lang") || "fr";
    applyLanguage(currentLang);

    const langToggleBtn = document.getElementById("langToggleBtn");
    if (langToggleBtn) {
        langToggleBtn.addEventListener("click", () => {
            currentLang = currentLang === "fr" ? "en" : "fr";
            localStorage.setItem("spiritty_lang", currentLang);
            applyLanguage(currentLang);
            loadScenario(activeScenario);
        });
    }

    // 3.2 Installer Tabs (spiritty.ai vs raw github)
    const installTabs = document.querySelectorAll(".installer-tab");
    const installCmdEl = document.getElementById("installCommand");
    const cmdOfficial = "curl -fsSL https://spiritty.ai/install.sh | bash";
    const cmdGithub = "curl -fsSL https://raw.githubusercontent.com/xorne-git/Spiritty/main/install.sh | bash";

    installTabs.forEach(tab => {
        tab.addEventListener("click", () => {
            installTabs.forEach(t => t.classList.remove("active"));
            tab.classList.add("active");
            const target = tab.dataset.target;
            if (target === "official") {
                installCmdEl.textContent = cmdOfficial;
            } else {
                installCmdEl.textContent = cmdGithub;
            }
        });
    });

    // 3.3 One-Click Copy Functionality
    const copyBtn = document.getElementById("copyInstallBtn");
    const toast = document.getElementById("toastMsg");

    if (copyBtn) {
        copyBtn.addEventListener("click", () => {
            const cmd = installCmdEl.textContent.trim();
            navigator.clipboard.writeText(cmd).then(() => {
                copyBtn.classList.add("copied");
                const copyText = copyBtn.querySelector(".btn-text");
                copyText.textContent = translations[currentLang].copied_btn;

                // Show toast
                toast.classList.add("show");
                setTimeout(() => {
                    toast.classList.remove("show");
                    copyBtn.classList.remove("copied");
                    copyText.textContent = translations[currentLang].copy_btn;
                }, 2400);
            }).catch(err => {
                console.error("Clipboard copy failed: ", err);
            });
        });
    }

    // 3.4 Interactive TUI Demo Scenario Switcher
    const scenarioBtns = document.querySelectorAll(".scenario-btn");
    scenarioBtns.forEach(btn => {
        btn.addEventListener("click", () => {
            scenarioBtns.forEach(b => b.classList.remove("active"));
            btn.classList.add("active");
            activeScenario = btn.dataset.scenario;
            loadScenario(activeScenario);
        });
    });

    // Load initial scenario
    loadScenario("perf");

    // 3.5 Navbar Scroll Effect
    const navbar = document.querySelector(".navbar");
    window.addEventListener("scroll", () => {
        if (window.scrollY > 40) {
            navbar.classList.add("scrolled");
        } else {
            navbar.classList.remove("scrolled");
        }
    });
});

// ==============================================================================
// 4. HELPER FUNCTIONS
// ==============================================================================

function applyLanguage(lang) {
    const dict = translations[lang];
    if (!dict) return;

    // Update all elements with data-i18n attribute
    document.querySelectorAll("[data-i18n]").forEach(el => {
        const key = el.getAttribute("data-i18n");
        if (dict[key]) {
            el.innerHTML = dict[key];
        }
    });

    // Update Language Toggle Button indicator
    const langBtnText = document.getElementById("langBtnText");
    if (langBtnText) {
        langBtnText.textContent = lang === "fr" ? "🇬🇧 EN" : "🇫🇷 FR";
    }

    // Update HTML lang attribute
    document.documentElement.lang = lang;
}

function loadScenario(scenarioId) {
    const data = demoScenarios[scenarioId];
    if (!data) return;

    const lang = document.documentElement.lang || "fr";

    // Top Header Elements
    const demoLeftLines = document.getElementById("demoLeftLines");
    const demoHostTab = document.getElementById("demoHostTab");
    const demoRightLines = document.getElementById("demoRightLines");

    if (demoLeftLines) demoLeftLines.textContent = data.left_lines;
    if (demoHostTab) demoHostTab.textContent = data.right_host;
    if (demoRightLines) demoRightLines.textContent = data.right_lines;

    // Left Panel Flow
    const demoChatFlow = document.getElementById("demoChatFlow");
    if (demoChatFlow) {
        const content = (data.chat_flow && data.chat_flow[lang]) ? data.chat_flow[lang] : data.chat_flow["fr"];
        demoChatFlow.innerHTML = content.trim();
    }

    // Right Panel Shell Content
    const demoTerminalContent = document.getElementById("demoTerminalContent");
    if (demoTerminalContent) {
        demoTerminalContent.innerHTML = data.term_content.trim();
    }

    // Footer Status Bar Elements
    const demoFooterModel = document.getElementById("demoFooterModel");
    const demoFooterReasoning = document.getElementById("demoFooterReasoning");
    const demoFooterTok = document.getElementById("demoFooterTok");
    const demoFooterCost = document.getElementById("demoFooterCost");
    const demoFooterCtx = document.getElementById("demoFooterCtx");
    const demoFooterApprove = document.getElementById("demoFooterApprove");

    if (demoFooterModel) demoFooterModel.textContent = data.footer_model;
    if (demoFooterReasoning) demoFooterReasoning.textContent = data.footer_reasoning;
    if (demoFooterTok) demoFooterTok.textContent = data.footer_tok;
    if (demoFooterCost) demoFooterCost.textContent = data.footer_cost;
    if (demoFooterCtx) demoFooterCtx.textContent = data.footer_ctx;
    if (demoFooterApprove) demoFooterApprove.textContent = data.footer_approve;
}
