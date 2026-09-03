use super::I18nKey;

pub fn translate(key: I18nKey) -> &'static str {
    match key {
        // Config Modal
        I18nKey::ConfigModalTitle => " ⚙️ Configuration Spiritty ",
        I18nKey::ConfigFieldProvider => "1. Fournisseur        ❯ ",
        I18nKey::ConfigFieldAutoApprove => "2. Auto-Approbation   ❯ ",
        I18nKey::ConfigFieldTheme => "3. Thème Interface    ❯ ",
        I18nKey::ConfigFieldModel => "4. Modèle IA          ❯ ",
        I18nKey::ConfigFieldApiUrl => "5. URL endpoint API   ❯ ",
        I18nKey::ConfigFieldApiKey => "6. Clé d'API          ❯ ",
        I18nKey::ConfigButtonSave => "  Enregistrer  ",
        I18nKey::ConfigNavNavigate => "Naviguer",
        I18nKey::ConfigNavValidateOrOpen => "Valider ou Ouvrir Liste",
        I18nKey::ConfigNavClose => "Fermer",
        I18nKey::ConfigPlaceholderSelectModel => "Sélectionner un modèle...",
        I18nKey::ConfigPlaceholderDefaultUrl => "(URL officielle par défaut)",
        I18nKey::ConfigPlaceholderNoKeyRequired => "(Aucune clé requise pour ce provider)",
        I18nKey::ConfigDropdownTitle => " ▾ Modèles Disponibles ",
        I18nKey::ConfigDropdownAddTitle => "＋ Ajouter un nouveau modèle :",
        I18nKey::ConfigDropdownEditTitle => "✎ Modifier le nom du modèle :",
        I18nKey::ConfigDropdownConfirm => "Confirmer",
        I18nKey::ConfigDropdownCancel => "Annuler",
        I18nKey::ConfigDropdownActionAdd => "Ajouter",
        I18nKey::ConfigDropdownActionEdit => "Éditer",
        I18nKey::ConfigDropdownActionDelete => "Supprimer",
        I18nKey::ConfigDropdownTagActive => " (Actif)",
        I18nKey::ConfigActionUpdatePricing => "Tarifs en ligne",
        I18nKey::PricingUpdateSuccess => "✓ Tarifs LLM synchronisés avec succès depuis Internet",
        I18nKey::PricingUpdateFailed => "✗ Échec de la mise à jour des tarifs en ligne",

        // Help Modal
        I18nKey::HelpModalTitle => " ⌨️  Raccourcis & Commandes Clés ",
        I18nKey::HelpSectionNavigation => "🧭 Navigation & Interface",
        I18nKey::HelpSectionAgent => "🤖 Agent IA & Actions",
        I18nKey::HelpSectionSessions => "📁 Sessions & Hôtes",
        I18nKey::HelpSectionGeneral => "⚙️ Contrôle & Aide",
        I18nKey::HelpKeyShift => "Shift",
        I18nKey::HelpKeyCtrl => "Ctrl",
        I18nKey::HelpKeyAlt => "Alt",
        I18nKey::HelpKeyTab => "Tab",
        I18nKey::HelpKeySpace => "Espace",
        I18nKey::HelpKeyOr => "  ou  ",
        I18nKey::HelpKeyMouseClick => "🖱 Clic",
        I18nKey::HelpKeyScroll => "🖱 Molette / PgUp/Dn",
        I18nKey::HelpKeyDrag => "🖱 Glisser bordure",
        I18nKey::HelpKeyClose => "Échap",
        I18nKey::HelpDescToggleFocus => "Basculer le focus (Chat ↔ Shell)",
        I18nKey::HelpDescMouseClick => "Focus direct sur le panneau cliqué",
        I18nKey::HelpDescScroll => "Défiler l'historique (Chat & Shell)",
        I18nKey::HelpDescResizePanels => "Ajuster la largeur des panneaux",
        I18nKey::HelpDescConfigModal => "Configuration des modèles & clés API",
        I18nKey::HelpDescSessionModal => "Historique & gestion des sessions",
        I18nKey::HelpDescNewSession => "Démarrer une nouvelle session",
        I18nKey::HelpDescAutoApprove => "Mode Auto-Approve (Safe / Sudo / YOLO)",
        I18nKey::HelpDescBookmarksModal => "Serveurs SSH & favoris Quick-Connect",
        I18nKey::HelpDescExportSession => "Exporter la session en Markdown",
        I18nKey::HelpDescMcpModal => "Serveurs & outils MCP (Protocol)",
        I18nKey::HelpDescChatSearch => "Rechercher dans l'historique du chat",
        I18nKey::HelpDescDiagnoseError => "Diagnostiquer & réparer l'erreur",
        I18nKey::HelpDescNewTab => "Ouvrir un nouvel onglet shell",
        I18nKey::HelpDescCloseTab => "Fermer l'onglet shell actif",
        I18nKey::HelpDescNextTab => "Basculer vers l'onglet suivant",
        I18nKey::HelpDescPrevTab => "Basculer vers l'onglet précédent",
        I18nKey::FooterApprovalLabel => "Approbation : ",
        I18nKey::HelpDescToggleHelp => "Ouvrir ou fermer cette aide",
        I18nKey::HelpDescQuit => "Quitter Spiritty",
        I18nKey::HelpDescCloseModal => "Fermer la modale active",
        I18nKey::HelpFooterPromptPrefix => "Appuyez sur ",
        I18nKey::HelpFooterPromptMiddle => " ou ",
        I18nKey::HelpFooterPromptSuffix => " pour fermer",

        // Sessions Modal
        I18nKey::SshReconnectTitle => " 🔗 Reconnexion SSH ",
        I18nKey::SshReconnectBody => "Cette session était connectée à :",
        I18nKey::SshReconnectConfirm => "⏎ Se reconnecter",
        I18nKey::SshReconnectLater => "Esc Plus tard",
        I18nKey::SessionModalTitle => " 🗂️ Gestionnaire de Sessions ",
        I18nKey::SessionHeaderTitle => "Sujet / Titre",
        I18nKey::SessionHeaderModel => "Modèle",
        I18nKey::SessionHeaderMessages => "Msgs",
        I18nKey::SessionHeaderTokens => "Tokens",
        I18nKey::SessionHeaderDate => "Date & Heure",
        I18nKey::SessionTagActive => "● Active",
        I18nKey::SessionActionLoad => "Charger",
        I18nKey::SessionActionNew => "Nouvelle",
        I18nKey::SessionActionCompact => "Compacter",
        I18nKey::SessionActionDelete => "Supprimer",
        I18nKey::SessionActionClose => "Fermer",
        I18nKey::SessionEmptyList => "Aucune session enregistrée pour le moment.",
        I18nKey::SessionConfirmDelete => "Confirmer la suppression ? (o/n)",

        // Chat Panel & UI
        I18nKey::ChatHeaderTitle => " Assistant IA ",
        I18nKey::TerminalHeaderTitle => " Terminal Shell ",
        I18nKey::ChatInputPlaceholder => "Posez une question ou demandez une commande...",
        I18nKey::ChatThinking => "Spiritty réfléchit...",
        I18nKey::ChatThoughtCompleted => "╭─ 💭 Réflexion (repliée) ──────────────────────────",
        I18nKey::ChatThoughtStreaming => "╭─ 💭 Réflexion en cours...",
        I18nKey::ChatWelcomeTitle => "Bienvenue sur Spiritty !",
        I18nKey::ChatWelcomeSubtitle => {
            "Posez vos questions système ou demandez de l'aide sur votre terminal."
        }

        // SSH & Host Management
        I18nKey::SshDetected => "Session SSH détectée vers",
        I18nKey::SshProfileLoaded => "Profil serveur chargé",
        I18nKey::SshScanPrompt => {
            "Appuyez sur [Alt + S] pour scanner l'environnement de ce serveur."
        }
        I18nKey::SshScanSuccess => "Environnement serveur profilé avec succès et enregistré.",
        I18nKey::SshScanFailed => "Échec de l'analyse de l'hôte distant.",
        I18nKey::SshLocalRestored => "Retour à l'environnement local.",
        I18nKey::HelpDescScanHost => {
            "Scanner et enregistrer l'environnement du serveur SSH distant"
        }

        // Agent System Prompts
        I18nKey::AgentLanguageInstruction => {
            "Réponds toujours en français de façon concise et technique."
        }
    }
}
