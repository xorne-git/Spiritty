# AGENTS.md — Directives de Développement pour Spiritty

Spiritty : binaire TUI Rust (ratatui + crossterm + tokio) combinant un agent IA sysadmin (panneau gauche) et un shell PTY interactif (panneau droit) en split-screen. Docs de référence : [ARCHITECTURE.md](ARCHITECTURE.md) (invariants de conception), [ROADMAP.md](ROADMAP.md) (jalons), [CHANGELOG.md](CHANGELOG.md).

---

## 🛠️ Commandes de vérification

```bash
cargo check                # compilation rapide
cargo clippy -- -D warnings   # zéro warning toléré
cargo test                 # toute la suite (logique pure, tourne headless)
cargo test --test session_test   # un seul fichier d'intégration
cargo build --release      # binaire standalone dans target/release/spiritty
```

- Pas de `rustfmt.toml` ni config clippy custom : valeurs par défaut.
- Les tests n'automatisent pas la TUI ni le PTY : ce sont des tests de logique pure (unitaires `#[cfg(test)]` dans les modules + intégration par domaine dans `tests/`). La validation visuelle TUI (redimensionnement, focus, raccourcis) se fait manuellement.

---

## 🏛️ Invariants non négociables

1. **Ne jamais bloquer la boucle d'événements** : tout traitement long (streaming LLM, I/O PTY, inspection système) part dans une tâche `tokio::spawn`. La communication inter-sous-systèmes (UI, PTY, agent) passe exclusivement par des messages typés `tokio::sync::mpsc` (voir `src/event.rs`).
2. **Human-in-the-loop** : aucune commande n'est injectée dans le PTY sans consentement explicite (hors mode YOLO activé par l'utilisateur). La classification des risques vit dans `src/agent/safety.rs` (`CommandRisk`: Safe / Sudo / Risky).
3. **i18n obligatoire** : toute chaîne visible (UI, modales, statuts, messages d'erreur) et tout system prompt passent par `src/i18n/`. Le catalogue `I18nKey` est typé : ajouter une chaîne = ajouter la variante d'enum **et** l'entrée dans `fr.rs` **et** `en.rs`, sinon `cargo check` échoue. ⚠️ Le fallback par défaut est le **français** (`Language::Fr` dans `src/i18n/mod.rs`), contrairement à ce que dit ARCHITECTURE.md §6.
4. **Réactivité PTY** : la frappe dans le shell droit doit rester indiscernable d'un terminal natif (zéro allocation inutile dans la boucle `crossterm::event`).
5. **Erreurs** : pas de `unwrap()`/`expect()` en dehors des tests ; propager via `thiserror`/`anyhow`. Les erreurs runtime deviennent des événements typés, jamais un crash de la TUI.

---

## 🗺️ Carte des modules (état réel)

```
src/
├── main.rs            # Bootstrap terminal + panic/signal hooks + event loop (pas de logique métier)
├── lib.rs             # Expose tous les modules (les tests d'intégration passent par la lib)
├── app.rs / event.rs  # État global + routeur d'événements central (clavier, PTY, LLM, timers)
├── cli.rs             # Parsing CLI fait maison (pas de clap) : --ssh, --model, --yolo, -s/-c...
├── agent/             # Agent IA : prompt.rs, tools.rs, safety.rs (classification commandes)
│   ├── providers/     # ollama.rs, gemini.rs, anthropic.rs, openai.rs
│   └── mcp/           # Client MCP (manager + processus)
├── pty/               # mod.rs (abstraction), process.rs (cycle de vie $SHELL), vt.rs (pont vt100→ratatui)
├── ui/                # mod.rs (layout), chat_panel.rs, terminal_panel.rs, theme.rs, components/ (modales)
├── system/            # hosts.rs (profils SSH/hosts.json), process_watcher.rs, clipboard.rs
├── session/           # Persistance ~/.config/spiritty/sessions/ + compactage de contexte
├── pricing/           # Coûts tokens (assets/pricing.json)
├── i18n/              # mod.rs (enum I18nKey), fr.rs, en.rs
└── config/            # ~/.config/spiritty/config.toml + surcharge system_prompt.md
```

- **Providers LLM** : toute marque OpenAI-compatible (DeepSeek, Z.ai/GLM, Grok, LM Studio...) passe par `providers/openai.rs` — ne pas créer un fichier par marque. Les modèles « raisonneurs » y gèrent le champ `reasoning_content` (replié en blocs `<think>…</think>`).
- **Contexte SSH** : le profiling des serveurs distants (détection `/proc`, cache `hosts.json`) adapte le system prompt à la distribution distante — voir `src/system/hosts.rs`.

---

## 🚀 Releases (rituel exact)

1. Rentrer les changements dans la section **« Non publié »** de `CHANGELOG.md` au fil de l'eau (catégories : `Ajouté` · `Modifié` · `Corrigé` · `Performance` · `Sécurité`).
2. Au release : bump `version` dans `Cargo.toml`, renommer « Non publié » en `## vX.Y.Z — YYYY-MM-DD`, ouvrir une nouvelle section « Non publié » vide, commit `chore(release): vX.Y.Z — version bump, CHANGELOG and installer fallback`.
3. Un tag `v*` déclenche `.github/workflows/release.yml` : builds Linux x86_64 + aarch64 (via `cross`) et macOS aarch64 uniquement (les runners Intel macOS sont retirés — ne pas ré-ajouter `x86_64-apple-darwin`).
4. Messages de commit en convention classique avec scope : `feat(session):`, `fix(agent):`, `ci(release):`, `docs(roadmap):`...

---

## 🔄 Règles de flux de travail

1. Consulter [ARCHITECTURE.md](ARCHITECTURE.md) avant toute refonte ; les plans d'implémentation détaillés vivent dans `docs/plans/YYYY-MM-DD_nom.md`.
2. Tenir [ROADMAP.md](ROADMAP.md) et la section « Non publié » du CHANGELOG à jour au fil des tâches.
3. **Ne JAMAIS faire de `git commit` ou `git push` de sa propre initiative** — uniquement à la demande expresse de l'utilisateur.
