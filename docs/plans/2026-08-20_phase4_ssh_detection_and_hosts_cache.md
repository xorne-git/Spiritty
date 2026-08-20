# Plan d'Implémentation — Détection Dynamique des Sessions SSH & Adaptation Multi-Serveurs (Phase 4)

**Date :** 20 Août 2026  
**Branche :** `feat/ssh-host-profiling`  
**Statut :** En cours d'implémentation

---

## 🎯 Contexte & Objectif
Lors de l'administration d'une flotte de serveurs/VPS distants via le shell intégré de Spiritty, l'agent IA doit savoir en temps réel sur quelle machine l'utilisateur opère.
Ce plan met en place :
1. **La détection en direct des sessions SSH** actives dans le PTY via l'arbre de processus `/proc/<pid>`.
2. **Un cache persistant de profils de serveurs** dans `~/.config/spiritty/hosts.json` (OS, distribution, noyau, gestionnaires de paquets, services).
3. **L'adaptation dynamique du contexte système** injecté dans le *System Prompt* de l'agent LLM (privilégiant les commandes de la distribution distante, ex: `apt` sur Debian au lieu de `pacman` sur Arch/CachyOS).
4. **Un retour visuel dans l'en-tête du terminal** (`🌐 SSH: root@vps-01 (Debian 12)`).
5. **Une commande de scan d'empreinte rapide (`Alt + S`)** pour profiler et mémoriser instantanément un nouveau serveur.

---

## 📐 Architecture & Modules

### 1. Module de Détection des Processus (`src/system/process_watcher.rs`) [NEW]
* Détecte le processus actif au premier plan sous le shell PTY (`child_pid`).
* Identifie si un processus `ssh`, `mosh-client` ou `sftp` est actif.
* Extrait la cible (`[user@]hostname`, port éventuel).
* Type d'état `ActiveSession` : `Local` ou `Ssh { target: String, command: Option<String> }`.

### 2. Gestionnaire de Profils d'Hôtes & Cache (`src/system/hosts.rs`) [NEW]
* Structure `HostProfile` : `target`, `hostname`, `distro`, `kernel`, `package_managers`, `init_system`, `user`, `last_seen`.
* Structure `HostsStore` avec sérialisation/désérialisation JSON (`~/.config/spiritty/hosts.json`).
* Parseur de sortie de sonde système (`parse_probe_output`) pour convertir les métadonnées récoltées en profil structuré.

### 3. Contexte Système Dynamique (`src/system/mod.rs`) [MODIFY]
* Intégration de `ActiveSession` et `Option<HostProfile>` dans `SystemContext`.
* Mise à jour de `to_prompt_context()` pour formater le contexte soit de la machine locale, soit du serveur distant connecté.

### 4. PTY Process (`src/pty/process.rs`) [MODIFY]
* Exposition de `pub fn child_pid(&self) -> Option<u32>`.

### 5. Application State & Événements (`src/app.rs` & `src/event.rs`) [MODIFY]
* Surveillance périodique de la session active (toutes les ~400ms sur tick ou à l'exécution de commande).
* Gestion du basculement d'environnement `Local <-> SSH`.
* Déclencheur du scan d'hôte `Alt + S` (`AppEvent::ScanHost` / `scan_remote_host()`).
* Notification Toast lors de la détection ou de la découverte d'un profil serveur.

### 6. Rendu UI (`src/ui/terminal_panel.rs` & `src/ui/mod.rs`) [MODIFY]
* En-tête de la fenêtre shell :
  * Si local : `💻 Ghostty`
  * Si SSH : `🌐 SSH: user@host (Distro)` avec badge mis en valeur.
* Barre d'état (Footer) : Affiche la cible active et l'aide `[Alt+S] Scan VPS` si le serveur n'est pas encore profilé.

### 7. Clés de Traduction (`src/i18n/`) [MODIFY]
* Ajout des clés pour les notifications SSH, scan de serveur et en-têtes.

---

## 🧪 Plan de Test
- `tests/hosts_test.rs` :
  - Test de sérialisation / désérialisation de `hosts.json`.
  - Test du parseur de probe output (`parse_probe_output`).
  - Test de basculement de `SystemContext` (Local vs SSH).
- `cargo test` : Vérification des 21+ tests unitaires existants + nouveaux tests.
- `cargo clippy -- -D warnings` : 0 warning.
