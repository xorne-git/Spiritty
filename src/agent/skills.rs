use std::path::{Path, PathBuf};

use crate::config::{SkillSelectionMode, SkillsConfig};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkillSource {
    Builtin,
    Global,
    Project,
}

impl SkillSource {
    pub fn label(&self) -> &'static str {
        match self {
            SkillSource::Builtin => "Built-in",
            SkillSource::Global => "Global",
            SkillSource::Project => "Project",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkillState {
    Auto,
    Enabled,
    Disabled,
}

#[derive(Debug, Clone)]
pub struct Skill {
    pub id: String,
    pub name: String,
    pub description: String,
    pub triggers: Vec<String>,
    pub content: String,
    pub source: SkillSource,
    pub file_path: Option<PathBuf>,
}

impl Skill {
    pub fn matches_query(&self, text: &str) -> bool {
        // Normalise : minuscules + tout caractère non-alphanumérique -> espace.
        // "digit, ssh-stop" devient " digit  ssh stop " -> le trigger "sh" ne matche plus.
        let normalized: String = text
            .to_lowercase()
            .chars()
            .map(|c| if c.is_alphanumeric() { c } else { ' ' })
            .collect();
        let padded = format!(" {normalized} ");
        self.triggers.iter().any(|trigger| {
            let needle = format!(" {} ", trigger.to_lowercase());
            padded.contains(&needle)
        })
    }
}

pub struct SkillsManager {
    skills: Vec<Skill>,
}

impl Default for SkillsManager {
    fn default() -> Self {
        Self::new()
    }
}

impl SkillsManager {
    pub fn new() -> Self {
        let mut manager = Self {
            skills: builtin_skills(),
        };
        manager.load_custom_skills();
        manager
    }

    pub fn skills(&self) -> &[Skill] {
        &self.skills
    }

    pub fn reload(&mut self) {
        self.skills = builtin_skills();
        self.load_custom_skills();
    }

    pub fn resolve_state(&self, skill_id: &str, config: &SkillsConfig) -> SkillState {
        if config.disabled.contains(&skill_id.to_string()) {
            SkillState::Disabled
        } else if config.enabled.contains(&skill_id.to_string()) {
            SkillState::Enabled
        } else {
            SkillState::Auto
        }
    }

    pub fn is_active_for_query(
        &self,
        skill: &Skill,
        config: &SkillsConfig,
        query: &str,
    ) -> bool {
        let state = self.resolve_state(&skill.id, config);
        match config.mode {
            SkillSelectionMode::Auto => match state {
                SkillState::Enabled => true,
                SkillState::Disabled => false,
                SkillState::Auto => skill.matches_query(query),
            },
            SkillSelectionMode::Manual => state == SkillState::Enabled,
        }
    }

    pub fn build_prompt_section(
        &self,
        config: &SkillsConfig,
        recent_messages: &[crate::app::ChatMessage],
    ) -> String {
        if self.skills.is_empty() {
            return String::new();
        }

        // Aggregate recent user messages to check triggers
        let mut query_haystack = String::new();
        for msg in recent_messages.iter().rev().take(4) {
            if msg.role == crate::app::MessageRole::User {
                query_haystack.push(' ');
                query_haystack.push_str(&msg.content);
            }
        }

        let mut active_skills = Vec::new();
        let mut inactive_skills = Vec::new();

        for skill in &self.skills {
            if self.is_active_for_query(skill, config, &query_haystack) {
                active_skills.push(skill);
            } else {
                inactive_skills.push(skill);
            }
        }

        let mut out = String::new();

        if !active_skills.is_empty() {
            out.push_str("\n\nSPECIALIZED SKILLS & DIRECTIVES:\n");
            out.push_str(
                "Follow these domain-specific directives tailored to the user's active tasks:\n\n",
            );
            for s in active_skills {
                out.push_str(&format!(
                    "### Skill: {} ({})\n{}\n\n",
                    s.id,
                    s.name,
                    s.content.trim()
                ));
            }
        }

        if !inactive_skills.is_empty() && config.mode == SkillSelectionMode::Auto {
            out.push_str("\nAVAILABLE STANDBY SKILLS:\n");
            out.push_str("These skills are available and will automatically activate when relevant topics are raised:\n");
            for s in inactive_skills {
                let triggers_summary = s.triggers.join(", ");
                out.push_str(&format!(
                    "- `{}`: {} (triggers: {})\n",
                    s.id, s.description, triggers_summary
                ));
            }
            out.push('\n');
        }

        out
    }

    fn load_custom_skills(&mut self) {
        // 1. Load global skills (~/.config/spiritty/skills/)
        if let Some(config_dir) = dirs::config_dir() {
            let global_path = config_dir.join("spiritty").join("skills");
            self.scan_directory(&global_path, SkillSource::Global);
        }

        // 2. Load workspace/project skills (.spiritty/skills/ in current working dir)
        if let Ok(current_dir) = std::env::current_dir() {
            let project_path = current_dir.join(".spiritty").join("skills");
            self.scan_directory(&project_path, SkillSource::Project);
        }
    }

    fn scan_directory(&mut self, dir: &Path, source: SkillSource) {
        if !dir.is_dir() {
            return;
        }

        let entries = match std::fs::read_dir(dir) {
            Ok(e) => e,
            Err(_) => return,
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() && path.extension().and_then(|ext| ext.to_str()) == Some("md") {
                if let Some(skill) = parse_skill_file(&path, source) {
                    // Replace or add
                    self.skills.retain(|s| s.id != skill.id);
                    self.skills.push(skill);
                }
            } else if path.is_dir() {
                let skill_md = path.join("SKILL.md");
                if skill_md.is_file() {
                    if let Some(skill) = parse_skill_file(&skill_md, source) {
                        self.skills.retain(|s| s.id != skill.id);
                        self.skills.push(skill);
                    }
                }
            }
        }
    }
}

pub fn parse_skill_file(path: &Path, source: SkillSource) -> Option<Skill> {
    let content = std::fs::read_to_string(path).ok()?;
    let fallback_id = path
        .file_stem()
        .and_then(|s| s.to_str())
        .map(|s| {
            if s.eq_ignore_ascii_case("SKILL") {
                path.parent()
                    .and_then(|p| p.file_name())
                    .and_then(|n| n.to_str())
                    .unwrap_or("custom-skill")
            } else {
                s
            }
        })
        .unwrap_or("custom-skill")
        .to_string();

    let (frontmatter, body) = split_frontmatter(&content);

    let mut id = fallback_id;
    let mut name = id.clone();
    let mut description = String::new();
    let mut triggers = Vec::new();

    if let Some(fm) = frontmatter {
        for line in fm.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some((key, val)) = line.split_once(':') {
                let key = key.trim().to_lowercase();
                let val = val.trim();
                match key.as_str() {
                    "id" | "name" if key == "id" => id = val.trim_matches('"').trim_matches('\'').to_string(),
                    "name" => name = val.trim_matches('"').trim_matches('\'').to_string(),
                    "description" => {
                        description = val.trim_matches('"').trim_matches('\'').to_string();
                    }
                    "triggers" => {
                        triggers = parse_list_value(val);
                    }
                    _ => {}
                }
            }
        }
    }

    if triggers.is_empty() {
        triggers.push(id.clone());
    }

    Some(Skill {
        id,
        name,
        description,
        triggers,
        content: body.to_string(),
        source,
        file_path: Some(path.to_path_buf()),
    })
}

fn split_frontmatter(content: &str) -> (Option<&str>, &str) {
    let trimmed = content.trim_start();
    if !trimmed.starts_with("---") {
        return (None, content);
    }

    let after_first = &trimmed[3..];
    if let Some(end_idx) = after_first.find("---") {
        let frontmatter = &after_first[..end_idx];
        let body = after_first[end_idx + 3..].trim_start_matches('\n');
        (Some(frontmatter), body)
    } else {
        (None, content)
    }
}

fn parse_list_value(raw: &str) -> Vec<String> {
    let trimmed = raw.trim();
    if trimmed.starts_with('[') && trimmed.ends_with(']') {
        let inside = &trimmed[1..trimmed.len() - 1];
        inside
            .split(',')
            .map(|s| s.trim().trim_matches('"').trim_matches('\'').to_string())
            .filter(|s| !s.is_empty())
            .collect()
    } else {
        trimmed
            .split(',')
            .map(|s| s.trim().trim_matches('"').trim_matches('\'').to_string())
            .filter(|s| !s.is_empty())
            .collect()
    }
}

pub fn builtin_skills() -> Vec<Skill> {
    vec![
        Skill {
            id: "shell-guru".to_string(),
            name: "Shell POSIX & Bash Expert".to_string(),
            description: "Directives d'écriture robuste de commandes shell, quoting strict, pipelines et gestion d'erreurs.".to_string(),
            triggers: vec![
                "bash".to_string(),
                "sh".to_string(),
                "shell".to_string(),
                "script".to_string(),
                "pipeline".to_string(),
                "pipe".to_string(),
                "subshell".to_string(),
                "heredoc".to_string(),
                "quoting".to_string(),
                "exit code".to_string(),
                "trap".to_string(),
                "xargs".to_string(),
            ],
            content: r#"# Directives d'Expertise Shell POSIX & Bash

1. **Quoting Strict & Sécurité des Variables :**
   - Toujours entourer les variables de guillemets doubles : `"$VAR"`, `"${VAR:-default}"`.
   - Ne jamais laisser de variable non citée dans un argument pour éviter le word-splitting et l'expansion de glob inattendue.
   - Utiliser `printf '%s\n' "$var"` plutôt que `echo "$var"` lorsque le contenu peut contenir des tirets ou des caractères d'échappement.

2. **Pipelines et Sous-shells Robustes :**
   - Dans les scripts et commandes composées, activer `set -euo pipefail` pour stopper immédiatement l'exécution en cas d'erreur masquée par un pipe.
   - Utiliser la substitution de processus `<(commande)` ou `>(commande)` pour éviter les fichiers temporaires inutiles.
   - Pour itérer sur des fichiers dont le nom peut contenir des espaces ou des sauts de ligne, utiliser `find ... -print0 | while IFS= read -r -d '' file; do ...; done` ou `xargs -0`.

3. **Portabilité et Bonnes Pratiques :**
   - Préférer `command -v cmd >/dev/null 2>&1` à `which cmd` pour tester la présence d'un binaire.
   - Remplacer les anciens backticks par la syntaxe standard et imbriquable `$(cmd)`.
   - Toujours vérifier le code de retour `$?` ou enchaîner avec `&&` / `||` pour une gestion d'erreur explicite."#.to_string(),
            source: SkillSource::Builtin,
            file_path: None,
        },
        Skill {
            id: "git-cli".to_string(),
            name: "Git CLI & Sauvetage de Dépôt".to_string(),
            description: "Commandes Git avancées, résolution de conflits, reflog, stash, worktrees et rebase sécurisé.".to_string(),
            triggers: vec![
                "git".to_string(),
                "commit".to_string(),
                "branch".to_string(),
                "rebase".to_string(),
                "merge".to_string(),
                "conflict".to_string(),
                "reflog".to_string(),
                "stash".to_string(),
                "worktree".to_string(),
                "cherry-pick".to_string(),
                "reset".to_string(),
                "head".to_string(),
            ],
            content: r#"# Directives d'Expertise Git CLI

1. **Sauvetage et Récupération d'Historique :**
   - Avant toute manipulation destructive sur l'historique, vérifier l'état avec `git status -s` et `git stash`.
   - En cas de perte apparente d'un commit ou après un `reset --hard` accidentel, utiliser immédiatement `git reflog` pour repérer le hash perdu (`HEAD@{1}`) et restaurer avec `git checkout -b rescue-branch <hash>`.
   - Ne JAMAIS recommander `git push --force` sans proposer l'alternative sécurisée `git push --force-with-lease`.

2. **Résolution de Conflits et Rebases :**
   - Lors d'un conflit de rebase ou merge, inspecter précisément les fichiers en conflit avec `git diff --name-only --diff-filter=U`.
   - Utiliser `git checkout --ours <file>` ou `git checkout --theirs <file>` lorsque l'intention est claire pour un fichier entier.
   - Si un rebase devient incontrôlable, guider l'utilisateur pour abandonner proprement sans dommage : `git rebase --abort`.

3. **Efficacité Avancée :**
   - Préférer `git worktree add ../feature-branch feature-branch` au clonage multiple pour travailler en parallèle sur plusieurs branches.
   - Nettoyage sécurisé : toujours exécuter un dry-run avec `git clean -nd` avant de supprimer des fichiers non suivis avec `git clean -fd`."#.to_string(),
            source: SkillSource::Builtin,
            file_path: None,
        },
        Skill {
            id: "process-triage".to_string(),
            name: "Diagnostic Processus & Ressources".to_string(),
            description: "Analyse de la consommation CPU/RAM, processus bloqués (D), signaux Linux, lsof et /proc.".to_string(),
            triggers: vec![
                "ps".to_string(),
                "kill".to_string(),
                "pid".to_string(),
                "pkill".to_string(),
                "pgrep".to_string(),
                "top".to_string(),
                "htop".to_string(),
                "processus".to_string(),
                "process".to_string(),
                "cpu".to_string(),
                "ram".to_string(),
                "memoire".to_string(),
                "mémoire".to_string(),
                "memory".to_string(),
                "zombie".to_string(),
                "load".to_string(),
                "oom".to_string(),
                "lsof".to_string(),
            ],
            content: r#"# Directives d'Expertise Processus & Ressources

1. **Localisation des Gros Consommateurs :**
   - Top 10 CPU : `ps -eo pid,ppid,user,%cpu,%mem,cmd --sort=-%cpu | head -n 11`.
   - Top 10 RAM : `ps -eo pid,ppid,user,%cpu,%mem,cmd --sort=-%mem | head -n 11`.
   - Vérifier la pression mémoire et le swap avec `free -h` et `vmstat 1 5`.

2. **Gestion Propre des Signaux :**
   - Toujours tenter un arrêt gracieux avec `SIGTERM` (`kill -15 <pid>` ou `pkill -15 <nom>`) avant de recourir au signal forcé `SIGKILL` (`kill -9 <pid>`).
   - Vérifier si le processus s'est arrêté proprement avant d'envoyer SIGKILL.
   - Identifier le parent d'un processus zombie ou orphelin (`ps -o ppid= -p <pid>`) : un processus zombie (état `Z`) ne peut pas être tué directement, il faut envoyer le signal à son parent ou attendre son adoption par init/systemd.

3. **Inspection Avancée (`/proc` et Fichiers Ouverts) :**
   - Retrouver la ligne de commande exacte d'un PID : `tr '\0' ' ' < /proc/<pid>/cmdline`.
   - Retrouver le répertoire de travail d'un PID : `readlink -f /proc/<pid>/cwd`.
   - Identifier les fichiers et sockets ouverts par un PID : `lsof -p <pid>` ou `ls -l /proc/<pid>/fd`."#.to_string(),
            source: SkillSource::Builtin,
            file_path: None,
        },
        Skill {
            id: "text-processing".to_string(),
            name: "Manipulation de Texte & Logs (awk, sed, jq)".to_string(),
            description: "Filtrage haute performance, parsing de logs, agrégations et manipulations de flux sans dépendance lourde.".to_string(),
            triggers: vec![
                "awk".to_string(),
                "sed".to_string(),
                "jq".to_string(),
                "grep".to_string(),
                "cut".to_string(),
                "sort".to_string(),
                "uniq".to_string(),
                "tr".to_string(),
                "log".to_string(),
                "logs".to_string(),
                "parsing".to_string(),
                "regex".to_string(),
                "csv".to_string(),
                "json".to_string(),
            ],
            content: r#"# Directives d'Expertise Manipulation de Texte & Logs

1. **Extraction et Agrégation Rapide de Logs :**
   - Compter et trier les occurrences fréquentes : `grep 'PATTERN' access.log | cut -d' ' -f1 | sort | uniq -c | sort -nr | head -n 20`.
   - Avec `awk` pour sommer ou calculer des moyennes : `awk '{sum += $NF; count++} END {if (count>0) print sum/count}' logfile`.
   - Préférer `rg` (ripgrep) pour sa vitesse si installé, sinon `grep -E` avec des motifs précis.

2. **Utilisation Sécurisée de `sed` :**
   - Lors de la modification de chemins de fichiers ou d'URLs, utiliser un délimiteur alternatif tel que `|` ou `,` pour éviter d'échapper chaque slash : `sed 's|/var/www|/srv/www|g'`.
   - Toujours tester la substitution sans `-i` (in-place) ou utiliser un suffixe de sauvegarde (`sed -i.bak '...'`) avant d'écraser le fichier source.

3. **Traitement Structuré JSON avec `jq` :**
   - Toujours utiliser `jq -r` (raw output) pour extraire des chaînes sans guillemets superflus dans des scripts shell.
   - Filtrer des objets par condition : `jq -r '.items[] | select(.status == "Error") | .name'`.
   - Aplatir des tableaux : `jq -c '.[]'`."#.to_string(),
            source: SkillSource::Builtin,
            file_path: None,
        },
        Skill {
            id: "network-tools".to_string(),
            name: "Diagnostic Réseau & Connectivité CLI".to_string(),
            description: "Inspection des interfaces, routes, ports d'écoute, requêtes DNS, latence et sockets avec ip, ss, curl, dig.".to_string(),
            triggers: vec![
                "ip".to_string(),
                "ss".to_string(),
                "network".to_string(),
                "reseau".to_string(),
                "réseau".to_string(),
                "dns".to_string(),
                "ping".to_string(),
                "curl".to_string(),
                "dig".to_string(),
                "tcpdump".to_string(),
                "nc".to_string(),
                "netcat".to_string(),
                "traceroute".to_string(),
                "route".to_string(),
                "gateway".to_string(),
                "interface".to_string(),
                "socket".to_string(),
            ],
            content: r#"# Directives d'Expertise Réseau & Connectivité

1. **Outils Linux Modernes (`ip` et `ss`) :**
   - Préférer systématiquement `ip` et `ss` aux anciens utilitaires dépréciés `ifconfig` et `netstat`.
   - Voir les adresses IP et statuts des interfaces : `ip -brief addr`.
   - Voir la table de routage et la passerelle par défaut : `ip route show`.
   - Vérifier quel processus écoute sur quels ports TCP/UDP : `ss -tulpn`.

2. **Tests de Connectivité et Diagnostic Port :**
   - Tester l'accessibilité d'un port distant avec timeout : `nc -zv -w 3 <host> <port>` ou en bash pur : `timeout 3 bash -c '</dev/tcp/<host>/<port>' 2>/dev/null && echo "Port ouvert"`.
   - Tester les requêtes HTTP avec en-têtes et codes retour : `curl -ILs -o /dev/null -w "%{http_code} (%{time_total}s)\n" <url>`.
   - Tracer la route IP : `traceroute -n <cible>` ou `ip route get <cible>` pour inspecter l'interface de sortie choisie par le kernel.

3. **Résolution DNS :**
   - Interroger directement un serveur DNS spécifique : `dig @1.1.1.1 +short <domaine>`.
   - Inspecter les enregistrements complets et le TTL : `dig +nocmd <domaine> any +multiline +noall +answer`."#.to_string(),
            source: SkillSource::Builtin,
            file_path: None,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builtin_skills_completeness() {
        let skills = builtin_skills();
        assert_eq!(skills.len(), 5);
        assert!(skills.iter().any(|s| s.id == "shell-guru"));
        assert!(skills.iter().any(|s| s.id == "git-cli"));
        assert!(skills.iter().any(|s| s.id == "process-triage"));
        assert!(skills.iter().any(|s| s.id == "text-processing"));
        assert!(skills.iter().any(|s| s.id == "network-tools"));

        for s in &skills {
            assert!(!s.name.is_empty());
            assert!(!s.description.is_empty());
            assert!(!s.triggers.is_empty());
            assert!(!s.content.is_empty());
            assert_eq!(s.source, SkillSource::Builtin);
        }
    }

    #[test]
    fn test_skill_query_matching() {
        let skills = builtin_skills();
        let git_skill = skills.iter().find(|s| s.id == "git-cli").unwrap();
        assert!(git_skill.matches_query("Comment faire un rebase interactif ?"));
        assert!(git_skill.matches_query("git status"));
        assert!(!git_skill.matches_query("installer nginx sur debian"));

        let process_skill = skills.iter().find(|s| s.id == "process-triage").unwrap();
        assert!(process_skill.matches_query("le serveur a un cpu à 100%, quel PID consomme ?"));
        assert!(process_skill.matches_query("comment tuer un zombie avec kill ?"));
    }

    #[test]
    fn test_frontmatter_parser() {
        let raw = r#"---
name: My Custom Skill
description: A great custom skill
triggers: [custom, test, helper]
---
# Custom Directives
Do this and that.
"#;
        let (fm, body) = split_frontmatter(raw);
        assert!(fm.is_some());
        assert!(body.starts_with("# Custom Directives"));

        let triggers = parse_list_value("[custom, test, helper]");
        assert_eq!(triggers, vec!["custom", "test", "helper"]);
    }

    #[test]
    fn test_skills_manager_auto_vs_manual() {
        let manager = SkillsManager {
            skills: builtin_skills(),
        };

        // Auto mode
        let auto_config = SkillsConfig {
            mode: SkillSelectionMode::Auto,
            enabled: Vec::new(),
            disabled: Vec::new(),
        };
        let prompt_auto = manager.build_prompt_section(
            &auto_config,
            &[crate::app::ChatMessage {
                role: crate::app::MessageRole::User,
                content: "comment nettoyer mon commit git ?".to_string(),
                command_proposal: None,
                attachments: Vec::new(),
            }],
        );
        assert!(prompt_auto.contains("Skill: git-cli"));
        assert!(prompt_auto.contains("AVAILABLE STANDBY SKILLS"));

        // Manual mode with nothing enabled
        let manual_config = SkillsConfig {
            mode: SkillSelectionMode::Manual,
            enabled: Vec::new(),
            disabled: Vec::new(),
        };
        let prompt_manual = manager.build_prompt_section(
            &manual_config,
            &[crate::app::ChatMessage {
                role: crate::app::MessageRole::User,
                content: "comment nettoyer mon commit git ?".to_string(),
                command_proposal: None,
                attachments: Vec::new(),
            }],
        );
        assert!(!prompt_manual.contains("Skill: git-cli"));

        // Manual mode with shell-guru enabled
        let manual_config_with_guru = SkillsConfig {
            mode: SkillSelectionMode::Manual,
            enabled: vec!["shell-guru".to_string()],
            disabled: Vec::new(),
        };
        let prompt_manual2 = manager.build_prompt_section(
            &manual_config_with_guru,
            &[crate::app::ChatMessage {
                role: crate::app::MessageRole::User,
                content: "bonjour".to_string(),
                command_proposal: None,
                attachments: Vec::new(),
            }],
        );
        assert!(prompt_manual2.contains("Skill: shell-guru"));
    }
}
