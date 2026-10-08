use super::I18nKey;

pub fn translate(key: I18nKey) -> &'static str {
    match key {
        // Config Modal
        I18nKey::ConfigModalTitle => " ⚙️ Configuration Spiritty ",
        I18nKey::ConfigFieldProvider => "1. Fournisseur        ❯ ",
        I18nKey::ConfigFieldAutoApprove => "2. Auto-Approbation   ❯ ",
        I18nKey::ConfigFieldTheme => "3. Thème Interface    ❯ ",
        I18nKey::ConfigFieldVoice => "4. Entrée Vocale      ❯ ",
        I18nKey::ConfigVoiceEnabled => "Activée (ON)",
        I18nKey::ConfigVoiceDisabled => "Désactivée (OFF)",
        I18nKey::ConfigFieldModel => "5. Modèle IA          ❯ ",
        I18nKey::ConfigFieldReasoning => "6. Réflexion IA       ❯ ",
        I18nKey::ConfigFieldApiUrl => "7. URL endpoint API   ❯ ",
        I18nKey::ConfigFieldApiKey => "8. Clé d'API          ❯ ",
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
        I18nKey::ConfigActionRefreshModels => "Actualiser modèles",
        I18nKey::ConfigRefreshInProgress => "⏳ Actualisation des modèles...",
        I18nKey::ConfigRefreshSuccess => "✓ Modèles synchronisés depuis l'API",
        I18nKey::ConfigRefreshFailed => "✗ Échec d'actualisation des modèles",
        I18nKey::ConfigRefreshMissingKey => "⚠️ Clé d'API requise pour ce fournisseur",
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
        I18nKey::HelpDescResizePanels => "Ajuster la séparation chat / terminal (←/→ ou ↑/↓)",
        I18nKey::HelpDescToggleOrientation => {
            "Basculer horizontal (haut/bas) / vertical (côte à côte)"
        }
        I18nKey::HelpDescSwapPanels => "Inverser l'ordre des panneaux (chat ↔ terminal)",
        I18nKey::HelpDescVoiceSegment => "Dicter un segment vocal (enregistrer → transcrire, 100 % local)",
        I18nKey::HelpDescVoiceContinuous => "Dictée continue (silence → envoi auto, 100 % local)",
        I18nKey::HelpDescConfigModal => "Configuration des modèles & clés API",
        I18nKey::HelpDescSessionModal => "Historique & gestion des sessions",
        I18nKey::HelpDescNewSession => "Démarrer une nouvelle session",
        I18nKey::HelpDescAutoApprove => "Mode Auto-Approve (Safe / Sudo / YOLO)",
        I18nKey::HelpDescBookmarksModal => "Serveurs SSH & favoris Quick-Connect",
        I18nKey::HelpDescExportSession => "Exporter la session en Markdown",
        I18nKey::HelpDescMcpModal => "Serveurs & outils MCP (Protocol)",
        I18nKey::HelpDescSkillsModal => "Gestionnaire de skills & directives (Ctrl+S)",
        I18nKey::HelpDescChatSearch => "Rechercher dans l'historique du chat",
        I18nKey::HelpDescDiagnoseError => "Diagnostiquer & réparer l'erreur",
        I18nKey::HelpDescNewTab => "Ouvrir un nouvel onglet shell",
        I18nKey::HelpDescCloseTab => "Fermer l'onglet shell actif",
        I18nKey::HelpDescNextTab => "Basculer vers l'onglet suivant",
        I18nKey::HelpDescPrevTab => "Basculer vers l'onglet précédent",
        I18nKey::FooterApprovalLabel => "Approbation : ",
        I18nKey::FooterVoiceLabel => "Voix",
        I18nKey::HelpDescToggleHelp => "Ouvrir ou fermer cette aide",
        I18nKey::HelpDescQuit => "Quitter Spiritty",
        I18nKey::HelpDescCloseModal => "Fermer la modale active",
        I18nKey::HelpFooterPromptPrefix => "Appuyez sur ",
        I18nKey::HelpFooterPromptMiddle => " ou ",
        I18nKey::HelpFooterPromptSuffix => " pour fermer",
        I18nKey::HelpFooterScroll => "Défiler",

        // Help Modal Slash Commands
        I18nKey::HelpSectionSlashCommands => "⚡ Commandes Slash (Prompt)",
        I18nKey::HelpKeySlashSsh => "/ssh [hôte]",
        I18nKey::HelpKeySlashExport => "/export [chemin]",
        I18nKey::HelpKeySlashSearch => "/search [texte]",
        I18nKey::HelpKeySlashRename => "/rename [titre]",
        I18nKey::HelpKeySlashApprove => "/approve [niveau]",
        I18nKey::HelpKeySlashTab => "/... + Tab",
        I18nKey::HelpDescSlashHelp => "Ouvrir cette modale d'aide",
        I18nKey::HelpDescSlashSkills => "Gestionnaire de skills & directives",
        I18nKey::HelpDescSlashBookmarks => "Favoris SSH ou connexion directe",
        I18nKey::HelpDescSlashExport => "Exporter la session en Markdown",
        I18nKey::HelpDescSlashSearch => "Rechercher dans l'historique du chat",
        I18nKey::HelpDescSlashNextPrevTab => "Naviguer vers l'onglet suivant / précédent",
        I18nKey::HelpDescSlashRename => "Renommer l'onglet actif du terminal",
        I18nKey::HelpDescSlashApprove => "Niveau d'approbation (safe / sudo / yolo / off)",
        I18nKey::HelpDescSlashVoice => "Activer ou désactiver la dictée vocale",
        I18nKey::HelpDescSlashTab => "Autocompléter la commande (ou afficher suggestions)",

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
        I18nKey::ChatPinnedPromptPrefix => "Vous",
        I18nKey::ChatPinnedCommandPrefix => "Commande",

        // SSH & Host Management
        I18nKey::SshDetected => "Session SSH détectée vers",
        I18nKey::SshProfileLoaded => "Profil serveur chargé",
        I18nKey::SshScanPrompt => {
            "Appuyez sur [Alt + S] pour scanner l'environnement de ce serveur."
        }
        I18nKey::SshScanSuccess => "Environnement serveur profilé avec succès et enregistré.",
        I18nKey::SshScanFailed => "Échec de l'analyse de l'hôte distant.",
        I18nKey::SshLocalRestored => "Retour à l'environnement local.",
        I18nKey::HelpDescScanHost => "Scanner et mémoriser l'hôte SSH distant",
        I18nKey::TabRenameTitle => "Renommer l'onglet",
        I18nKey::TabRenamePrompt => "Nouveau nom de l'onglet :",
        I18nKey::TabRenameHelp => "[Entrée] Valider · [Échap] Annuler (vide = réinitialiser)",
        I18nKey::HelpDescRenameTab => "Renommer l'onglet actif du terminal",

        // Layout / split toasts
        I18nKey::ToastLayoutHorizontal => "Affichage horizontal : Chat en haut, Terminal en bas",
        I18nKey::ToastLayoutVertical => "Affichage vertical : Chat à gauche, Terminal à droite",
        I18nKey::ToastLayoutVerticalSwapped => {
            "Affichage vertical : Terminal à gauche, Chat à droite"
        }
        I18nKey::ToastLayoutHorizontalSwapped => {
            "Affichage horizontal : Terminal en haut, Chat en bas"
        }

        // Local voice input
        I18nKey::VoiceDisabledHint => {
            "Entrée vocale désactivée : activez [voice] dans la configuration"
        }
        I18nKey::VoiceRecordingToast => "Enregistrement… appuyez à nouveau sur F8 pour arrêter",
        I18nKey::VoiceTranscribingToast => "Transcription locale en cours…",
        I18nKey::VoiceTranscribedHint => "Transcription insérée dans le prompt (F7/F8 pour redicter)",
        I18nKey::VoiceError => "Erreur d'entrée vocale : ",
        I18nKey::VoiceContinuousOnToast => "Dictée continue activée (parlez, le silence valide)",
        I18nKey::VoiceContinuousOffToast => "Dictée continue désactivée",
        I18nKey::VoiceBadgeRecording => "● REC",
        I18nKey::VoiceBadgeTranscribing => "⟳ Transcription…",

        // Agent System Prompts
        I18nKey::AgentLanguageInstruction => {
            "Réponds toujours en français de façon concise et technique."
        }

        // Safety & Auto-Approve
        I18nKey::AutoApproveMaxConsecutiveReached => {
            "Arrêt de sécurité : limite de commandes consécutives auto-approuvées atteinte"
        }

        // Skills Management Modal
        I18nKey::SkillsModalTitle => "🎯 Skills & Directives d'Expertise [Ctrl+S]",
        I18nKey::SkillsModeAuto => "Mode : ⚡ Auto (Détection intelligente par mots-clés)",
        I18nKey::SkillsModeManual => "Mode : ✋ Manuel (Sélection fixe par l'utilisateur)",
        I18nKey::SkillsModeHelp => "[Tab/M] Basculer mode",
        I18nKey::SkillsStateAuto => "AUTO",
        I18nKey::SkillsStateEnabled => "ON",
        I18nKey::SkillsStateDisabled => "OFF",
        I18nKey::SkillsSourceBuiltin => "Intégré",
        I18nKey::SkillsSourceGlobal => "Global",
        I18nKey::SkillsSourceProject => "Projet",
        I18nKey::SkillsHelpToggleState => "Basculer état",
        I18nKey::SkillsHelpToggleMode => "Mode",
        I18nKey::SkillsHelpNew => "Nouveau",
        I18nKey::SkillsHelpScroll => "Défiler",
        I18nKey::SkillsHelpClose => "Fermer",
        I18nKey::SkillsDetailTriggers => "Mots-clés déclencheurs :",
        I18nKey::SkillsDetailDescription => "Description :",
        I18nKey::SkillsDetailDirectives => "Directives appliquées à l'agent :",
        I18nKey::SkillsEmptyList => "Aucun skill disponible",
        I18nKey::SkillsNewModalTitle => "Créer un nouveau skill",
        I18nKey::SkillsNewPromptId => "Identifiant du skill (ex: k8s-debug) : ",
        I18nKey::SkillsCreatedSuccess => "Skill créé avec succès dans ~/.config/spiritty/skills/",
        I18nKey::SkillsTypeLabel => "Type : ",
        I18nKey::SkillsNewModalHint => "[ Entrée: Créer | Échap: Annuler ]",

        // Modal save toasts & slash commands
        I18nKey::ToastConfigSaved => "✓ Configuration enregistrée",
        I18nKey::ToastSkillsSaved => "✓ Configuration des skills enregistrée",
        I18nKey::ToastMcpSaved => "✓ Configuration MCP enregistrée",
        I18nKey::ToastBookmarksSaved => "✓ Favoris SSH enregistrés",
        I18nKey::ToastTabRenamed => "✓ Titre de l'onglet enregistré",
        I18nKey::ToastUnknownSlashCommand => "Commande inconnue. Tapez /help pour l'aide.",
        I18nKey::ToastPressCtrlCAgainToQuit => "Appuyez à nouveau sur Ctrl+C pour quitter",
        I18nKey::TerminalTooSmall => "Terminal trop petit — minimum 20x8",
    }
}
