use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

#[test]
fn test_vt_screen_rendering() {
    let mut parser = vt100::Parser::new(5, 80, 1000);
    for i in 0..20 {
        parser.process(format!("Line {}\r\n", i).as_bytes());
    }
    let old = parser.screen().scrollback();
    parser.set_scrollback(usize::MAX);
    let max_history = parser.screen().scrollback();
    parser.set_scrollback(old);
    println!("Max scrollback available: {}, old: {}", max_history, old);
}

#[test]
fn test_paragraph_wrapping_line_count() {
    use ratatui::style::{Color, Modifier, Style};
    use ratatui::text::{Line, Span};
    use ratatui::widgets::{Paragraph, Widget, Wrap};

    let sample_text = "Salut ! Je suis sous cachyOS avec dankshell + niri, peux tu vérifier que le service calendar est actif (il doit faire partie de dms.service) et qu'il se lance bien au boot?";
    let lines = vec![Line::from(sample_text)];
    let width = 40;

    let mut buf = Buffer::empty(Rect::new(0, 0, width, 50));
    let p = Paragraph::new(lines.clone()).wrap(Wrap { trim: false });
    p.render(Rect::new(0, 0, width, 50), &mut buf);

    // Find the last non-empty line in buffer
    let mut rendered_lines: u16 = 0;
    for y in 0..50 {
        let has_content =
            (0..width).any(|x| buf.cell((x, y)).map(|c| c.symbol() != " ").unwrap_or(false));
        if has_content {
            rendered_lines = y + 1;
        }
    }

    assert!(rendered_lines > 0);

    fn count_lines(lines: &[Line], width: u16) -> u16 {
        use unicode_width::UnicodeWidthStr;
        if width == 0 {
            return lines.len() as u16;
        }
        let max_w = width as usize;
        let mut total: u16 = 0;

        for line in lines {
            let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
            if text.is_empty() {
                total = total.saturating_add(1);
                continue;
            }

            let mut line_rows: u16 = 1;
            let mut current_col: usize = 0;

            for chunk in text.split_inclusive(' ') {
                let trimmed = chunk.trim_end_matches(' ');
                let content_w = trimmed.width();
                let trailing_spaces = chunk.len() - trimmed.len();

                if content_w == 0 {
                    // Only spaces
                    if current_col + trailing_spaces <= max_w {
                        current_col += trailing_spaces;
                    } else {
                        line_rows = line_rows.saturating_add(1);
                        current_col = 0;
                    }
                    continue;
                }

                if current_col + content_w <= max_w {
                    // Word content fits on current line!
                    if current_col + content_w + trailing_spaces <= max_w {
                        current_col += content_w + trailing_spaces;
                    } else {
                        // Fits exactly at the edge, trailing space is dropped
                        line_rows = line_rows.saturating_add(1);
                        current_col = 0;
                    }
                } else {
                    // Word content does not fit on current line, must wrap
                    if current_col > 0 {
                        line_rows = line_rows.saturating_add(1);
                    }

                    if content_w > max_w {
                        let mut rem = content_w;
                        while rem > max_w {
                            line_rows = line_rows.saturating_add(1);
                            rem -= max_w;
                        }
                        if rem + trailing_spaces <= max_w {
                            current_col = rem + trailing_spaces;
                        } else {
                            line_rows = line_rows.saturating_add(1);
                            current_col = 0;
                        }
                    } else if content_w + trailing_spaces <= max_w {
                        current_col = content_w + trailing_spaces;
                    } else {
                        line_rows = line_rows.saturating_add(1);
                        current_col = 0;
                    }
                }
            }
            total = total.saturating_add(line_rows);
        }

        total
    }

    let calculated = count_lines(&lines, width);
    println!(
        "Ratatui rendered: {}, Calculated: {}",
        rendered_lines, calculated
    );
    assert_eq!(rendered_lines, calculated);

    // Test 2: Complex multi-line conversation with code blocks, list items, and short lines
    let complex_lines = vec![
        Line::from("👤 Bonjour, peux-tu m'aider ?"),
        Line::from(""),
        Line::from("🧞 Oui bien sûr ! Voici ce que nous allons vérifier :"),
        Line::from("  - Point 1 : Vérifier le statut du service avec systemctl"),
        Line::from("  - Point 2 : Vérifier les journaux avec journalctl -xeu dms.service"),
        Line::from(""),
        Line::from("⚡ COMMANDE #1 Safe"),
        Line::from("  systemctl --user status dms.service"),
        Line::from("  ↵ Enter Exécuter"),
        Line::from(""),
        Line::from("💻 `systemctl --user status dms.service`"),
        Line::from(""),
        Line::from("🧞 Analyse terminée avec succès. Tout fonctionne parfaitement."),
    ];

    let mut buf2 = Buffer::empty(Rect::new(0, 0, width, 100));
    let p2 = Paragraph::new(complex_lines.clone()).wrap(Wrap { trim: false });
    p2.render(Rect::new(0, 0, width, 100), &mut buf2);

    let mut rendered_lines2: u16 = 0;
    for y in 0..100 {
        let has_content = (0..width).any(|x| {
            buf2.cell((x, y))
                .map(|c| c.symbol() != " ")
                .unwrap_or(false)
        });
        if has_content {
            rendered_lines2 = y + 1;
        }
    }

    let calculated2 = count_lines(&complex_lines, width);
    println!(
        "Ratatui rendered complex: {}, Calculated: {}",
        rendered_lines2, calculated2
    );
    assert_eq!(rendered_lines2, calculated2);

    // Test 3: Table and wide horizontal lines
    let table_lines = vec![
        Line::from("📋 Services utilisateur (systemd --user) - DMS inclus"),
        Line::from("│ # │ Service │ État │ Description │"),
        Line::from("├──────────┼───────────────┼─────────────┼─────────────────────────────────────────────────────────────┤"),
        Line::from("│ 1 │ dms.service │ ✅ ACTIVE │ Desktop Manager Service (gestion de la session graphique Niri) │"),
        Line::from("│ 2 │ pipewire.service │ ✅ ACTIVE │ Audio / MIDI / PortAudio │"),
        Line::from(""),
        Line::from("✅ Conclusion"),
        Line::from("• DMS n'est PAS un service système (systemctl) -> c'est un service utilisateur (systemctl --user)."),
        Line::from("• Il se lance avec le UID du propriétaire de la session graphique (souvent 1001 sous CachyOS/Niri)."),
        Line::from("• C'est tout à fait normal."),
        Line::from("Si DMS ne démarre pas, c'est souvent que niri-session.service n'a pas été lancé."),
    ];

    let mut buf3 = Buffer::empty(Rect::new(0, 0, width, 100));
    let p3 = Paragraph::new(table_lines.clone()).wrap(Wrap { trim: false });
    p3.render(Rect::new(0, 0, width, 100), &mut buf3);

    let mut rendered_lines3: u16 = 0;
    for y in 0..100 {
        let line_str: String = (0..width)
            .map(|x| buf3.cell((x, y)).map(|c| c.symbol()).unwrap_or(" "))
            .collect();
        if !line_str.trim().is_empty() {
            rendered_lines3 = y + 1;
            println!("Row {:02}: |{}|", y, line_str);
        }
    }

    for (idx, l) in table_lines.iter().enumerate() {
        let c = count_lines(std::slice::from_ref(l), width);
        println!("Line {:02} (count {}): {}", idx, c, l);
    }

    let calculated3 = count_lines(&table_lines, width);
    println!(
        "Ratatui rendered table: {}, Calculated: {}",
        rendered_lines3, calculated3
    );
    assert_eq!(rendered_lines3, calculated3);
    let callout_lines = vec![
        Line::from(vec![
            Span::styled("> ", Style::default().fg(Color::Yellow)),
            Span::styled("⚠️ Note : ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::styled("niri.service est le compositeur Wayland. Tous les autres services utilisateur s'exécutent dans la session graphique gérée par DMS (Dank Material Shell) et non directement par systemd classique — c'est tout à fait normal.", Style::default().fg(Color::White)),
        ]),
    ];

    let mut buf4 = Buffer::empty(Rect::new(0, 0, width, 50));
    let p4 = Paragraph::new(callout_lines.clone()).wrap(Wrap { trim: false });
    p4.render(Rect::new(0, 0, width, 50), &mut buf4);

    let mut rendered_lines4: u16 = 0;
    for y in 0..50 {
        let line_str: String = (0..width)
            .map(|x| buf4.cell((x, y)).map(|c| c.symbol()).unwrap_or(" "))
            .collect();
        if !line_str.trim().is_empty() {
            rendered_lines4 = y + 1;
            println!("Callout row {:02}: |{}|", y, line_str);
        }
    }

    let calculated4 = count_lines(&callout_lines, width);
    println!(
        "Ratatui rendered callout: {}, Calculated: {}",
        rendered_lines4, calculated4
    );
    assert_eq!(rendered_lines4, calculated4);
}

#[tokio::test]
async fn test_chat_scrolling_repro() {
    use spiritty::app::{App, ChatMessage, MessageRole};
    use spiritty::ui::chat_panel::ChatPanel;

    let (event_tx, _event_rx) = tokio::sync::mpsc::unbounded_channel();
    let mut app = App::new(event_tx, 55, 100).expect("create app");
    app.messages.clear();

    app.messages.push(ChatMessage {
        role: MessageRole::Assistant,
        content: "L'utilisateur me salue simplement. Je dois répondre de manière concise et amicale en français, comme un assistant Spiritty. Pas besoin d'exécuter de commande ici - c'est juste une salutation.\n\nSalut ! 🦀 Je suis Spiritty, ton assistant terminal. Comment puis-je t'aider aujourd'hui ?".to_string(),
        command_proposal: None,
        attachments: Vec::new(),
    });

    app.messages.push(ChatMessage {
        role: MessageRole::User,
        content: "Génère-moi un fichier ~/audit_systeme.md récapitulant les informations clés de mon noyau, shell et mémoire RAM.".to_string(),
        command_proposal: None,
        attachments: Vec::new(),
    });

    app.messages.push(ChatMessage {
        role: MessageRole::Assistant,
        content: "<think>\nL'utilisateur veut que je génère un fichier ~/audit_systeme.md avec des infos sur le noyau, le shell et la RAM. Je dois d'abord récupérer ces informations via tool:run_command, puis créer le fichier.\n\nJe vais exécuter les commandes nécessaires pour obtenir :\n- Version du noyau (uname -r)\n- Shell en cours (echo $SHELL ou whoami)\n- Informations sur la RAM (free -h)\n</think>\n```tool:run_command\nuname -r && echo \"---\" && whoami && echo \"---\" && free -h | head -2 && echo \"---\" && cat /etc/os-release | grep PRETTY_NAME\n```".to_string(),
        command_proposal: None,
        attachments: Vec::new(),
    });

    app.messages.push(ChatMessage {
        role: MessageRole::User,
        content: "💻 `uname -r && echo \"---\" && whoami && echo \"---\" && free -h | head -2 && echo \"---\" && cat /etc/os-release | grep PRETTY_NAME`".to_string(),
        command_proposal: None,
        attachments: Vec::new(),
    });

    app.messages.push(ChatMessage {
        role: MessageRole::Assistant,
        content: "```bash\ncat > ~/audit_systeme.md << 'EOF'\n# Audit Système - CachyOS\n\n## Informations Clés\n\n### Noyau (Kernel)\n- **Version** : 7.1.8-1-cachyos\n- **Distribution** : CachyOS\n\n### Shell\n- **Shell actif** : xorne (alias pour fish)\n- **Utilisateur** : xorne\n\n### Mémoire RAM\n| Statut | Quantité |\n|--------|----------|\n| **Total** | 31 GiB |\n| **Utilisé** | 11 GiB |\n| **Libre** | 885 MiB |\n| **Disponible** | 19 GiB |\n\n### Résumé\n- **Utilisation RAM** : ~36% (11 GiB / 31 GiB)\n- **Etat** : ✅ Bon - La mémoire est correctement gérée avec suffisamment de ressources disponibles.\nEOF\n```".to_string(),
        command_proposal: None,
        attachments: Vec::new(),
    });

    let panel_area_big = Rect::new(0, 0, 100, 200);
    let mut buf_big = Buffer::empty(panel_area_big);
    let panel_big = ChatPanel::new(&app);
    panel_big.render_panel(panel_area_big, &mut buf_big);

    let mut b_52 = Buffer::empty(Rect::new(0, 0, 100, 52));
    let panel = ChatPanel::new(&app);
    panel.render_panel(Rect::new(0, 0, 100, 52), &mut b_52);

    let rendered_text = (0..52)
        .map(|y| {
            (0..100)
                .map(|x| b_52.cell((x, y)).map(|c| c.symbol()).unwrap_or(" "))
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n");

    assert!(
        rendered_text.contains("Alt"),
        "The bottom action buttons [Alt + 1] must be visible in the chat viewport!"
    );
    assert!(
        rendered_text.contains("EOF"),
        "The command content EOF must be visible in the chat viewport!"
    );
}

#[tokio::test]
async fn test_chat_scroll_bounds() {
    use spiritty::app::App;
    let (event_tx, _event_rx) = tokio::sync::mpsc::unbounded_channel();
    let mut app = App::new(event_tx, 55, 100).expect("create app");

    assert_eq!(app.chat_scroll_from_bottom, 0);

    // Scrolling down while at the bottom stays clamped at 0 (no overscroll into void)
    app.scroll_chat_down(3);
    assert_eq!(app.chat_scroll_from_bottom, 0);

    app.scroll_chat_down(5);
    assert_eq!(app.chat_scroll_from_bottom, 0);

    // Scrolling up increases scroll_from_bottom
    app.scroll_chat_up(4);
    assert_eq!(app.chat_scroll_from_bottom, 4);

    app.scroll_chat_up(6);
    assert_eq!(app.chat_scroll_from_bottom, 10);

    // Scrolling down decreases smoothly down to 0
    app.scroll_chat_down(3);
    assert_eq!(app.chat_scroll_from_bottom, 7);

    // Resetting clears scroll
    app.reset_chat_scroll();
    assert_eq!(app.chat_scroll_from_bottom, 0);
}

#[tokio::test]
async fn test_empty_chat_badge_shows_zero() {
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;
    use spiritty::app::App;
    use spiritty::ui::chat_panel::ChatPanel;

    let (event_tx, _event_rx) = tokio::sync::mpsc::unbounded_channel();
    let app = App::new(event_tx, 55, 100).expect("create app");
    let mut buf = Buffer::empty(Rect::new(0, 0, 100, 50));

    let panel = ChatPanel::new(&app);
    panel.render_panel(Rect::new(0, 0, 100, 50), &mut buf);

    let top_header = (0..100)
        .map(|x| buf.cell((x, 0)).map(|c| c.symbol()).unwrap_or(" "))
        .collect::<String>();

    assert!(
        top_header.contains("0 l."),
        "Empty chat panel header must show 0 l. instead of 4 l. (got: {})",
        top_header
    );
}

#[test]
fn test_markdown_heading_rendering() {
    let md = "# Title 1\n## Title 2\n### Title 3\n#### Title 4\n##### Title 5\n###### Title 6\nNormal text";
    // Check that stripping hashes works without leaving literal markdown tags
    for line in md.lines() {
        let trimmed = line.trim_start();
        let is_heading = trimmed.starts_with('#');
        if is_heading {
            let clean = trimmed.trim_start_matches('#').trim_start();
            assert!(!clean.starts_with('#'));
        }
    }
}

#[test]
fn test_clean_multiline_command() {
    use spiritty::app::clean_multiline_command;

    let bad_cmd =
        "echo \"=== SERVICES ===\" && \\\nsystemctl list-units --type=service && \\\necho \"done\"";
    let cleaned = clean_multiline_command(bad_cmd);
    assert_eq!(
        cleaned,
        "echo \"=== SERVICES ===\" && systemctl list-units --type=service && echo \"done\""
    );
    assert!(!cleaned.contains("&& \\"));
    assert!(!cleaned.contains("\\ &&"));

    let pipeline_cmd = "echo \"=== Services ===\" && \\\n{ systemctl list-units; } | \\\nawk '{print $1}' | sort -u";
    let cleaned_pipe = clean_multiline_command(pipeline_cmd);
    assert_eq!(
        cleaned_pipe,
        "echo \"=== Services ===\" && { systemctl list-units; } | awk '{print $1}' | sort -u"
    );
    assert!(!cleaned_pipe.contains("| &&"));

    let script_cmd = "#!/usr/bin/env bash\nwhile IFS= read -r line; do\n  echo \"$line\"\ndone";
    let cleaned_script = clean_multiline_command(script_cmd);
    assert!(cleaned_script.contains("while IFS="));

    let heredoc_cmd = "cat > ~/audit_systeme.md << 'EOF'\n# Audit Système - CachyOS\n\n## Informations Clés\n- **Version** : 7.1.8-1-cachyos\nEOF\ncat ~/audit_systeme.md";
    let cleaned_heredoc = clean_multiline_command(heredoc_cmd);
    assert!(
        cleaned_heredoc.contains("# Audit Système - CachyOS"),
        "Markdown headings starting with # must not be stripped in heredocs"
    );
    assert!(cleaned_heredoc.contains("<< 'EOF'"));
    assert!(
        !cleaned_heredoc.contains("&& #"),
        "Heredoc body lines must not be joined with &&"
    );

    let bg_cmd = "npx @deepseek-ai/dsh web > /tmp/dsh.log 2>&1 &\necho \"PID: $!\"\nsleep 2\ncat /tmp/dsh.log";
    let cleaned_bg = clean_multiline_command(bg_cmd);
    assert_eq!(
        cleaned_bg,
        "npx @deepseek-ai/dsh web > /tmp/dsh.log 2>&1 & echo \"PID: $!\" && sleep 2 && cat /tmp/dsh.log"
    );
}

#[test]
fn test_sanitize_bash_command_syntax() {
    use spiritty::app::sanitize_bash_command_syntax;

    // Compound brace missing trailing semicolon
    let bad_compound = "{ echo \"# Title\" && echo \"done\" } > ~/file.md";
    let fixed = sanitize_bash_command_syntax(bad_compound);
    assert_eq!(fixed, "{ echo \"# Title\" && echo \"done\"; } > ~/file.md");

    // Already valid compound brace with semicolon
    let valid_compound = "{ echo \"a\"; echo \"b\"; } > ~/file.md";
    assert_eq!(sanitize_bash_command_syntax(valid_compound), valid_compound);

    // Parameter expansion ${VAR} must NOT be altered
    let param_exp = "echo \"Session: ${XDG_SESSION_TYPE:-unknown}\"";
    assert_eq!(sanitize_bash_command_syntax(param_exp), param_exp);

    // Awk single-quoted scripts must NOT be altered
    let awk_cmd = "awk '{print $1}'";
    assert_eq!(sanitize_bash_command_syntax(awk_cmd), awk_cmd);
}

#[tokio::test]
async fn test_stop_agent_generation() {
    use spiritty::app::{App, ChatMessage, MessageRole};
    let (event_tx, _event_rx) = tokio::sync::mpsc::unbounded_channel();
    let mut app = App::new(event_tx, 55, 100).expect("create app");

    app.messages.push(ChatMessage {
        role: MessageRole::Assistant,
        content: "Génération partielle...".to_string(),
        command_proposal: None,
        attachments: Vec::new(),
    });

    app.agent.is_generating = true;
    assert!(app.agent.is_generating);

    app.stop_agent_generation();

    assert!(!app.agent.is_generating);
    assert_eq!(
        app.messages.last().unwrap().content,
        "Génération partielle..."
    );
    assert!(app.toast_message.is_some());

    let _ = spiritty::session::SessionStorage::delete(&app.current_session.id);
}

#[test]
fn test_format_command_for_pty() {
    use spiritty::app::{format_command_for_pty, format_command_for_pty_with_session};

    // 1. Simple cd command
    assert_eq!(
        format_command_for_pty("cd /var/log", "fish"),
        " cd /var/log\n"
    );

    // 2. Simple single line in local fish (clean native command)
    assert_eq!(format_command_for_pty("free -h", "fish"), " free -h\n");

    // 3. Simple single line in local bash (clean native command)
    assert_eq!(format_command_for_pty("free -h", "bash"), " free -h\n");

    // 4. Bash-specific syntax (variable assignment BGPID=$! or heredocs) in fish -> wraps in bash -c
    assert_eq!(
        format_command_for_pty("node app.js & BGPID=$!", "fish"),
        " bash -c 'node app.js & BGPID=$!'\n"
    );

    // 5. Bash-specific syntax in bash -> executes directly
    assert_eq!(
        format_command_for_pty("node app.js & BGPID=$!", "bash"),
        " node app.js & BGPID=$!\n"
    );

    // 5b. Heredoc (multiline, `<<EOF`) on a LOCAL zsh -> wrapped in `bash -c '…'` so the
    //     interactive line editor never sees the body split across physical lines
    //     (regression: "cmdand heredoc> =" / `<<''EOF'>` corruption on real sessions).
    assert_eq!(
        format_command_for_pty(
            "sudo tee /etc/acpi <<'EOF'\n[Unit]\nDescription=acpi\nEOF",
            "zsh"
        ),
        " bash -c 'sudo tee /etc/acpi <<'\\''EOF'\\''\n[Unit]\nDescription=acpi\nEOF'\n"
    );

    // 5c. Non-heredoc bash syntax on local zsh stays native (no needless wrap).
    assert_eq!(
        format_command_for_pty("echo $? > /tmp/rc", "zsh"),
        " echo $? > /tmp/rc\n"
    );

    // 5d. Heredoc on a REMOTE shell is safely wrapped in `bash -c '…'` to prevent
    //     an `exit` from closing the remote SSH session.
    assert_eq!(
        format_command_for_pty_with_session(
            "printf 'x' | sudo tee /etc/f.txt << 'E'\nx\nE",
            "zsh",
            true,
            true
        ),
        " bash -c 'printf '\\''x'\\'' | sudo tee /etc/f.txt << '\\''E'\\''\nx\nE'\n"
    );

    // 6. Simple single line on remote SSH (tool capture -> clean command, no inline sentinel,
    //    so the remote shell doesn't echo the sentinel into the terminal)
    let remote_tool_cmd = format_command_for_pty_with_session("free -h", "fish", true, true);
    assert_eq!(remote_tool_cmd, " free -h\n");

    // 7. Simple single line on remote SSH (manual user Alt+1 -> pure clean command)
    let remote_user_cmd =
        format_command_for_pty_with_session("cat ~/audit_systeme.md", "fish", true, false);
    assert_eq!(remote_user_cmd, " cat ~/audit_systeme.md\n");
}

#[test]
fn test_repair_missing_heredoc_terminator() {
    use spiritty::app::repair_missing_heredoc_terminator;

    // 1. Missing EOF
    let truncated = "cat > ~/audit_systeme.md << 'EOF'\nAudit Système - CachyOS\nDate : 2026-08-20";
    let repaired = repair_missing_heredoc_terminator(truncated);
    assert_eq!(
        repaired,
        "cat > ~/audit_systeme.md << 'EOF'\nAudit Système - CachyOS\nDate : 2026-08-20\nEOF\n"
    );

    // 2. Missing ENDOFFILE
    let truncated2 = "cat << 'ENDOFFILE' > ~/file.txt\nSome content";
    let repaired2 = repair_missing_heredoc_terminator(truncated2);
    assert_eq!(
        repaired2,
        "cat << 'ENDOFFILE' > ~/file.txt\nSome content\nENDOFFILE\n"
    );

    // 3. Already closed EOF
    let valid = "cat > ~/audit.md << EOF\nContent\nEOF";
    let repaired_valid = repair_missing_heredoc_terminator(valid);
    assert_eq!(repaired_valid, valid);
}

#[test]
fn test_clean_heredoc_script() {
    use spiritty::app::clean_heredoc_script;

    let messy_raw = "pour récupérer ces infos.\ncat > ~/audit_systeme.md << 'EOF'\ntotal   utilisé  libre\nMem:   $(free -h)\nEOF\n✓ Fichier mis à jour avec les vraies données du système !\nVérifie le contenu avec : cat ~/audit_systeme.md\ncat ~/audit_systeme.md";
    let cleaned = clean_heredoc_script(messy_raw);
    assert_eq!(
        cleaned,
        "cat > ~/audit_systeme.md << 'EOF'\ntotal   utilisé  libre\nMem:   $(free -h)\nEOF\ncat ~/audit_systeme.md"
    );
    assert!(!cleaned.contains("pour récupérer"));
    assert!(!cleaned.contains("✓ Fichier"));
    assert!(!cleaned.contains("Vérifie le contenu"));
}

#[test]
fn test_clean_heredoc_script_drops_leading_comment() {
    use spiritty::app::clean_heredoc_script;

    // A leading `# comment` line is a complete (empty) bash command: once injected into the PTY,
    // it fires the completion sentinel before the heredoc runs, truncating the capture. It must
    // be stripped so the heredoc opener is the first line sent to the shell.
    let raw = "# 1. Filtre élargi (tous les POST wp-login.php, sans restriction de code)\nsudo tee /etc/fail2ban/filter.d/wp-auth.conf << 'EOF'\n[Definition]\nfailregex = ^<HOST> .* \"POST /wp-login\\.php HTTP/.*\"\nEOF";
    let cleaned = clean_heredoc_script(raw);
    assert!(
        cleaned.starts_with("sudo tee /etc/fail2ban/filter.d/wp-auth.conf << 'EOF'"),
        "leading comment should be dropped, got: {:?}",
        cleaned
    );
    assert!(!cleaned.contains("Filtre élargi"));
    // The heredoc body must be preserved verbatim.
    assert!(cleaned.contains("[Definition]"));
    assert!(cleaned.ends_with("EOF"));
}

#[test]
fn test_parse_command_execution_request() {
    use spiritty::app::parse_command_execution_request;

    // Direct affirmative phrases
    assert_eq!(parse_command_execution_request("ok", 3), Some(0));
    assert_eq!(parse_command_execution_request("oui", 3), Some(0));
    assert_eq!(parse_command_execution_request("oui vas y", 3), Some(0));
    assert_eq!(parse_command_execution_request("oui vas-y", 3), Some(0));
    assert_eq!(parse_command_execution_request("ok vas y", 3), Some(0));
    assert_eq!(parse_command_execution_request("oui stp", 3), Some(0));
    assert_eq!(parse_command_execution_request("oui bien sûr", 3), Some(0));
    assert_eq!(parse_command_execution_request("vas y", 3), Some(0));
    assert_eq!(parse_command_execution_request("fais le", 3), Some(0));
    assert_eq!(parse_command_execution_request("lance", 3), Some(0));
    assert_eq!(parse_command_execution_request("go", 3), Some(0));

    // Bare question marks and question-punctuated approvals ("?", "oui?", "ok?", "ouais ?")
    assert_eq!(parse_command_execution_request("?", 3), Some(0));
    assert_eq!(parse_command_execution_request("??", 3), Some(0));
    assert_eq!(parse_command_execution_request(" ? ", 3), Some(0));
    assert_eq!(parse_command_execution_request("oui?", 3), Some(0));
    assert_eq!(parse_command_execution_request("oui ?", 3), Some(0));
    assert_eq!(parse_command_execution_request("ok?", 3), Some(0));
    assert_eq!(parse_command_execution_request("ok ?", 3), Some(0));
    assert_eq!(parse_command_execution_request("ouais", 3), Some(0));
    assert_eq!(parse_command_execution_request("ouais ?", 3), Some(0));

    // Idiomatic French & English affirmative expressions with commas / punctuation
    assert_eq!(parse_command_execution_request("ok, vas y", 3), Some(0));
    assert_eq!(parse_command_execution_request("oui, vas-y", 3), Some(0));
    assert_eq!(parse_command_execution_request("c'est bon", 3), Some(0));
    assert_eq!(parse_command_execution_request("c bon", 3), Some(0));
    assert_eq!(parse_command_execution_request("c'est ok", 3), Some(0));
    assert_eq!(parse_command_execution_request("c ok", 3), Some(0));
    assert_eq!(parse_command_execution_request("ça marche", 3), Some(0));
    assert_eq!(parse_command_execution_request("ca marche", 3), Some(0));
    assert_eq!(parse_command_execution_request("lance la", 3), Some(0));
    assert_eq!(parse_command_execution_request("fais", 3), Some(0));
    assert_eq!(parse_command_execution_request("alors ?", 3), Some(0));
    assert_eq!(parse_command_execution_request("ok pour moi", 3), Some(0));
    assert_eq!(parse_command_execution_request("d'acc", 3), Some(0));
    assert_eq!(parse_command_execution_request("envoie", 3), Some(0));

    // Numbered commands
    assert_eq!(parse_command_execution_request("1", 3), Some(0));
    assert_eq!(parse_command_execution_request("2", 3), Some(1));
    assert_eq!(parse_command_execution_request("3", 3), Some(2));
    assert_eq!(parse_command_execution_request("lance 2", 3), Some(1));
    assert_eq!(parse_command_execution_request("lance la 3", 3), Some(2));
    assert_eq!(parse_command_execution_request("cmd 1", 3), Some(0));
    assert_eq!(parse_command_execution_request("commande 2", 3), Some(1));

    // Out of bounds / non-command prompts
    assert_eq!(parse_command_execution_request("4", 3), None);
    assert_eq!(
        parse_command_execution_request("comment installer nginx ?", 3),
        None
    );
    assert_eq!(
        parse_command_execution_request("pourquoi cette commande ?", 3),
        None
    );
    assert_eq!(
        parse_command_execution_request("c'est quoi ce script ?", 3),
        None
    );
    assert_eq!(parse_command_execution_request("non", 3), None);
    assert_eq!(parse_command_execution_request("attends", 3), None);
    assert_eq!(parse_command_execution_request("ok", 0), None);
    assert_eq!(parse_command_execution_request("?", 0), None);
}

#[test]
fn test_continuous_spinner() {
    use spiritty::ui::get_spinner_char;

    assert_eq!(get_spinner_char(0), "⣾");
    assert_eq!(get_spinner_char(1), "⣽");
    assert_eq!(get_spinner_char(7), "⣷");
    assert_eq!(get_spinner_char(8), "⣾");
}

#[test]
fn test_prompt_cursor_word_wrapping() {
    use unicode_width::UnicodeWidthStr;

    fn str_visual_width(s: &str) -> usize {
        s.width()
    }

    fn compute_prompt_cursor_and_lines(
        text: &str,
        cursor_byte_pos: usize,
        max_w: usize,
    ) -> (u16, u16, u16) {
        if max_w == 0 || text.is_empty() {
            return (0, 0, 1);
        }

        let mut current_row: u16 = 0;
        let mut current_col: usize = 0;
        let mut cursor_row: u16 = 0;
        let mut cursor_col: usize = 0;
        let mut cursor_found = false;

        let mut byte_offset: usize = 0;

        let sublines: Vec<&str> = text.split('\n').collect();

        for (sub_idx, subline) in sublines.iter().enumerate() {
            if sub_idx > 0 {
                if !cursor_found && byte_offset == cursor_byte_pos {
                    cursor_row = current_row;
                    cursor_col = current_col;
                    cursor_found = true;
                }
                byte_offset += 1; // for '\n'
                current_row = current_row.saturating_add(1);
                current_col = 0;
            }

            if subline.is_empty() {
                if !cursor_found && byte_offset == cursor_byte_pos {
                    cursor_row = current_row;
                    cursor_col = current_col;
                    cursor_found = true;
                }
                continue;
            }

            for word in subline.split_inclusive(' ') {
                let word_bytes = word.len();
                let word_trimmed = word.trim_end_matches(' ');
                let word_w = str_visual_width(word_trimmed);
                let trailing_spaces = word.len() - word_trimmed.len();

                if !cursor_found
                    && cursor_byte_pos >= byte_offset
                    && cursor_byte_pos <= byte_offset + word_bytes
                {
                    let inside_offset = cursor_byte_pos - byte_offset;
                    let inside_str = &word[..inside_offset];
                    let inside_w = str_visual_width(inside_str);

                    if current_col + word_w <= max_w || current_col == 0 {
                        cursor_row = current_row;
                        cursor_col = current_col + inside_w;
                    } else {
                        cursor_row = current_row.saturating_add(1);
                        cursor_col = inside_w;
                    }
                    cursor_found = true;
                }

                if word_w == 0 {
                    if current_col + trailing_spaces <= max_w {
                        current_col += trailing_spaces;
                    } else {
                        current_row = current_row.saturating_add(1);
                        current_col = 0;
                    }
                } else if current_col + word_w <= max_w {
                    if current_col + word_w + trailing_spaces <= max_w {
                        current_col += word_w + trailing_spaces;
                    } else {
                        current_row = current_row.saturating_add(1);
                        current_col = 0;
                    }
                } else {
                    if current_col > 0 {
                        current_row = current_row.saturating_add(1);
                    }

                    if word_w > max_w {
                        let mut rem = word_w;
                        while rem > max_w {
                            current_row = current_row.saturating_add(1);
                            rem -= max_w;
                        }
                        if rem + trailing_spaces <= max_w {
                            current_col = rem + trailing_spaces;
                        } else {
                            current_row = current_row.saturating_add(1);
                            current_col = 0;
                        }
                    } else if word_w + trailing_spaces <= max_w {
                        current_col = word_w + trailing_spaces;
                    } else {
                        current_row = current_row.saturating_add(1);
                        current_col = 0;
                    }
                }

                byte_offset += word_bytes;
            }
        }

        if !cursor_found {
            cursor_row = current_row;
            cursor_col = current_col;
        }

        let total_rows = current_row.saturating_add(1);
        (cursor_row, cursor_col as u16, total_rows)
    }

    let text = "Fais-moi un bilan complet de ma configuration graphique et Wayland : versions des pilotes Nvidia / Mesa, état de niri et écran, et utilisation VRAM actuelle sous forme de tableau.";
    let width = 74;

    let (c_row, c_col, t_rows) = compute_prompt_cursor_and_lines(text, text.len(), width);
    println!(
        "Prompt end cursor: row {}, col {}, total rows {}",
        c_row, c_col, t_rows
    );

    let (c_row2, c_col2, t_rows2) = compute_prompt_cursor_and_lines(text, text.len() - 1, width);
    println!(
        "Prompt after backspace cursor: row {}, col {}, total rows {}",
        c_row2, c_col2, t_rows2
    );

    assert_eq!(t_rows, 3);
    assert_eq!(t_rows2, 3);
    assert_eq!(c_row, 2);
    assert_eq!(c_row2, 2);
    assert_eq!(c_col2, c_col - 1);
}

#[tokio::test]
async fn test_shift_enter_multiline_prompt() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use spiritty::app::App;
    use tokio::sync::mpsc;

    let (tx, _rx) = mpsc::unbounded_channel();
    let mut app = App::new(tx, 24, 80).unwrap();
    app.focus = spiritty::app::Focus::Chat;

    // Type "Line 1"
    for c in "Line 1".chars() {
        app.handle_key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE));
    }
    assert_eq!(app.chat_input, "Line 1");

    // Press Shift+Enter -> inserts '\n'
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::SHIFT));
    assert_eq!(app.chat_input, "Line 1\n");

    // Type "Line 2"
    for c in "Line 2".chars() {
        app.handle_key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE));
    }
    assert_eq!(app.chat_input, "Line 1\nLine 2");

    // Press Ctrl+J -> inserts '\n'
    app.handle_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::CONTROL));
    assert_eq!(app.chat_input, "Line 1\nLine 2\n");
}

#[test]
fn test_repair_prematurely_closed_code_blocks() {
    use spiritty::app::{extract_all_command_proposals, repair_prematurely_closed_code_blocks};

    // Regression: the model emitted an empty ```bash fence then the real script *outside* any
    // fence. Repair must drop the spurious empty fence WITHOUT manufacturing a new ```bash
    // block from the residual text (that turned trailing prose/reasoning into fake commands).
    let glitched = "Je corrige avec le bon pattern :\n\n```bash\n \n```\nfor v in 7.4 8.4 8.5; do\n  sudo sed -i -e 's/a/b/' /etc/php/$v/fpm/php.ini\ndone\n\nsudo systemctl restart php-fpm`";
    let repaired = repair_prematurely_closed_code_blocks(glitched);

    assert!(
        !repaired.contains("```bash"),
        "repair must not manufacture a bash block: {repaired}"
    );
    assert!(repaired.contains("for v in 7.4 8.4 8.5; do"));
    assert!(
        !repaired.ends_with("```"),
        "repair must not fabricate a closing fence: {repaired}"
    );

    // The un-fenced residual text must not be promoted to a command proposal.
    let proposals = extract_all_command_proposals(glitched);
    assert!(
        proposals.is_empty(),
        "residual un-fenced text must not become a command proposal: {proposals:?}"
    );
}

#[test]
fn test_untagged_output_block_is_not_a_command_proposal() {
    use spiritty::app::extract_all_command_proposals;

    // The model quoted `free -h` output in an untagged code block as illustration. This must NOT
    // be extracted as a command proposal (it produced the spurious "Swap:  511Mi  0B  511Mi").
    let text = "✅ **Swap vidé avec succès**\n\n```\nSwap:  511Mi  0B  511Mi\n```\n\nLe swap est reparti à zéro.";
    let proposals = extract_all_command_proposals(text);
    assert!(
        proposals.is_empty(),
        "expected no proposals, got: {:?}",
        proposals
    );

    // A real untagged shell command must still be extracted.
    let cmd_text = "Voici la commande :\n\n```\nfree -h | head -n 3\n```";
    let cmd_proposals = extract_all_command_proposals(cmd_text);
    assert_eq!(cmd_proposals, vec!["free -h | head -n 3"]);
}

#[tokio::test]
async fn test_responsive_footer_rendering_at_various_widths() {
    use ratatui::{backend::TestBackend, Terminal};
    use spiritty::app::App;
    use tokio::sync::mpsc;

    let (tx, _rx) = mpsc::unbounded_channel();
    let mut app = App::new(tx, 24, 80).unwrap();

    // Test across various terminal widths: 60 (narrow), 80 (standard), 100 (medium), 140 (wide)
    let model_name = app.get_active_model_name();

    for width in [60, 80, 100, 140] {
        let backend = TestBackend::new(width, 24);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                spiritty::ui::draw(f, &mut app);
            })
            .unwrap();

        let buf = terminal.backend().buffer();
        let footer_y = 23;
        let mut footer_text = String::new();
        for x in 0..width {
            footer_text.push_str(buf.cell((x, footer_y)).map(|c| c.symbol()).unwrap_or(" "));
        }

        // 1. Left info: the model name is shown; the provider (icon + name) is intentionally
        //    omitted to leave room for the always-visible approval badge.
        assert!(
            !footer_text.contains("󰚩")
                && !footer_text.contains("Ollama")
                && !footer_text.contains("DeepSeek"),
            "Width {} must not show the provider anymore! Rendered: '{}'",
            width,
            footer_text
        );
        if width >= 80 {
            assert!(
                footer_text.contains(&model_name),
                "Width {} should contain the model name '{}'! Rendered: '{}'",
                width,
                model_name,
                footer_text
            );
        }

        // 2. Right shortcuts: F1 is prioritized and always present, all Ctrl+* are removed
        assert!(
            footer_text.contains("F1"),
            "Width {} should contain F1 shortcut! Rendered: '{}'",
            width,
            footer_text
        );
        assert!(
            !footer_text.contains("Ctrl +") && !footer_text.contains("Config"),
            "Width {} should not contain Ctrl+* shortcuts! Rendered: '{}'",
            width,
            footer_text
        );

        // 3. Wide terminals show thinking badge and full powerline badges for all features
        if width >= 80 {
            assert!(
                footer_text.contains('🧠'),
                "Width {} should contain thinking badge 🧠! Rendered: '{}'",
                width,
                footer_text
            );
        }

        if width >= 140 {
            // Priority shortcuts (F3 approval, F7/F8 voice, F4/F5 layout/switch, F1 help)
            for expected in ["F3", "F7/F8", "F4/F5", "F1", "Layout/Switch"] {
                assert!(
                    footer_text.contains(expected),
                    "Width {} should contain {}! Rendered: '{}'",
                    width,
                    expected,
                    footer_text
                );
            }
        }
    }
}


// ---------------------------------------------------------------------------
// Render-cache streaming regressions (chat panel, two-pass draw)
// ---------------------------------------------------------------------------

fn panel_text(buf: &ratatui::buffer::Buffer) -> String {
    let mut s = String::new();
    for y in 0..buf.area.height {
        for x in 0..buf.area.width {
            s.push_str(buf.cell((x, y)).map(|c| c.symbol()).unwrap_or(" "));
        }
        s.push('\n');
    }
    s
}

#[tokio::test]
async fn test_streaming_tail_is_rendered_live() {
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;
    use spiritty::app::{App, ChatMessage, MessageRole};
    use spiritty::ui::chat_panel::ChatPanel;

    // Regression: while `is_generating`, the tail assistant message must reach
    // the widget — a cache-missed tail used to vanish (blank scrolling rows).
    let (event_tx, _event_rx) = tokio::sync::mpsc::unbounded_channel();
    let mut app = App::new(event_tx, 55, 100).expect("create app");
    app.messages.clear();
    app.messages.push(ChatMessage {
        role: MessageRole::User,
        content: "Hello".to_string(),
        command_proposal: None,
        attachments: Vec::new(),
    });
    app.messages.push(ChatMessage {
        role: MessageRole::Assistant,
        content: "Réponse partielle en cours de streaming XYZQ".to_string(),
        command_proposal: None,
        attachments: Vec::new(),
    });
    app.agent.is_generating = true;
    app.spinner_frame = 3;

    let mut buf = Buffer::empty(Rect::new(0, 0, 100, 50));
    ChatPanel::new(&app).render_panel(Rect::new(0, 0, 100, 50), &mut buf);
    let text = panel_text(&buf);

    assert!(
        text.contains("Réponse partielle en cours de streaming XYZQ"),
        "streaming tail must be visible while generating; got:\n{text}"
    );
}

#[tokio::test]
async fn test_deep_thinking_wording_when_silent_stream() {
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;
    use spiritty::app::{App, ChatMessage, MessageRole};
    use spiritty::ui::chat_panel::ChatPanel;

    // Long silent thinking phase: loader + explicit wording, not a bare ghost.
    let (event_tx, _event_rx) = tokio::sync::mpsc::unbounded_channel();
    let mut app = App::new(event_tx, 55, 100).expect("create app");
    app.messages.clear();
    app.messages.push(ChatMessage {
        role: MessageRole::Assistant,
        content: String::new(),
        command_proposal: None,
        attachments: Vec::new(),
    });
    app.agent.is_generating = true;
    app.spinner_frame = 5;

    let mut buf = Buffer::empty(Rect::new(0, 0, 100, 50));
    ChatPanel::new(&app).render_panel(Rect::new(0, 0, 100, 50), &mut buf);
    let text = panel_text(&buf);

    let expected_fr = "Réflexion profonde";
    let expected_en = "Deep thinking";
    assert!(
        text.contains(expected_fr) || text.contains(expected_en),
        "silent stream must show deep-thinking wording; got:\n{text}"
    );
    assert!(
        text.contains("🧞"),
        "ghost marker must accompany the thinking wording; got:\n{text}"
    );
}

#[tokio::test]
async fn test_streaming_frame_stability_between_spins() {
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;
    use spiritty::app::{App, ChatMessage, MessageRole};
    use spiritty::ui::chat_panel::ChatPanel;

    // Two consecutive frames with different spinner frames must keep identical
    // message rows (only the loader glyph may differ) — cache/pass coherence.
    let (event_tx, _event_rx) = tokio::sync::mpsc::unbounded_channel();
    let mut app = App::new(event_tx, 55, 100).expect("create app");
    app.messages.clear();
    app.messages.push(ChatMessage {
        role: MessageRole::Assistant,
        content: "Corps stable pendant le streaming.".to_string(),
        command_proposal: None,
        attachments: Vec::new(),
    });
    app.agent.is_generating = true;

    let mut render_at = |spinner_frame: usize| -> String {
        app.spinner_frame = spinner_frame;
        let mut buf = Buffer::empty(Rect::new(0, 0, 100, 50));
        ChatPanel::new(&app).render_panel(Rect::new(0, 0, 100, 50), &mut buf);
        panel_text(&buf)
    };

    let a = render_at(0);
    let b = render_at(2);
    // The loader glyph legitimately rotates; message rows must not shift or drop.
    let normalize = |s: &str| {
        ["⣾", "⣽", "⣻", "⢿", "⡿", "⣟", "⣯", "⣷"]
            .iter()
            .fold(s.to_string(), |acc, g| acc.replace(g, "#"))
    };
    assert_eq!(
        normalize(&a),
        normalize(&b),
        "spinner tick must not shift or drop message rows"
    );
}

#[tokio::test]
async fn test_f10_fast_track_approves_pending_command() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use spiritty::app::{App, Focus, PendingToolApproval};

    let (event_tx, _event_rx) = tokio::sync::mpsc::unbounded_channel();
    let mut app = App::new(event_tx, 55, 100).expect("create app");

    // 1. F10 does nothing when nothing is pending (must not panic).
    app.handle_key(KeyEvent::new(KeyCode::F(10), KeyModifiers::NONE));

    // 2. F10 approves from the CHAT pane.
    let (tx, rx) = tokio::sync::oneshot::channel::<bool>();
    app.pending_tool_approval = Some(PendingToolApproval {
        command: "sudo cat /etc/fail2ban/jail.local".to_string(),
        approval_tx: Some(tx),
    });
    app.handle_key(KeyEvent::new(KeyCode::F(10), KeyModifiers::NONE));
    assert!(
        app.pending_tool_approval.is_none(),
        "pending must be consumed"
    );
    assert!(rx.await.expect("approval sent"), "F10 must approve");

    // 3. F10 approves from the TERMINAL pane too.
    let (tx2, rx2) = tokio::sync::oneshot::channel::<bool>();
    app.focus = Focus::Terminal;
    app.pending_tool_approval = Some(PendingToolApproval {
        command: "systemctl status fail2ban".to_string(),
        approval_tx: Some(tx2),
    });
    app.handle_key(KeyEvent::new(KeyCode::F(10), KeyModifiers::NONE));
    assert!(app.pending_tool_approval.is_none());
    assert!(rx2.await.expect("approval sent"));
}

#[tokio::test]
async fn test_enter_key_approves_pending_command_when_prompt_empty() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use spiritty::app::{App, Focus, PendingToolApproval};

    let (event_tx, _event_rx) = tokio::sync::mpsc::unbounded_channel();
    let mut app = App::new(event_tx, 55, 100).expect("create app");
    app.focus = Focus::Chat;

    // 1. Enter on empty prompt approves the pending command
    let (tx, rx) = tokio::sync::oneshot::channel::<bool>();
    app.pending_tool_approval = Some(PendingToolApproval {
        command: "apt update".to_string(),
        approval_tx: Some(tx),
    });
    app.chat_input.clear();
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(
        app.pending_tool_approval.is_none(),
        "pending must be consumed"
    );
    assert!(
        rx.await.expect("approval sent"),
        "Enter must approve when prompt is empty"
    );

    // 2. Enter with 'non' declines the command
    let (tx_no, rx_no) = tokio::sync::oneshot::channel::<bool>();
    app.pending_tool_approval = Some(PendingToolApproval {
        command: "rm -rf /tmp/test".to_string(),
        approval_tx: Some(tx_no),
    });
    app.chat_input = "non".to_string();
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(app.pending_tool_approval.is_none());
    assert!(
        !rx_no.await.expect("decline sent"),
        "Entering 'non' must decline"
    );
    assert!(app.chat_input.is_empty(), "chat_input must be cleared");

    // 3. Shift+Enter does NOT approve, but inserts newline
    let (tx_shift, mut rx_shift) = tokio::sync::oneshot::channel::<bool>();
    app.pending_tool_approval = Some(PendingToolApproval {
        command: "reboot".to_string(),
        approval_tx: Some(tx_shift),
    });
    app.chat_input = "attends".to_string();
    app.cursor_pos = 7;
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::SHIFT));
    assert!(
        app.pending_tool_approval.is_some(),
        "Shift+Enter must not consume pending"
    );
    assert!(
        rx_shift.try_recv().is_err(),
        "no approval must be sent on Shift+Enter"
    );
    assert_eq!(app.chat_input, "attends\n");

    // 4. Enter with a new question/instruction declines the pending command so prompt can be processed
    let (tx_query, rx_query) = tokio::sync::oneshot::channel::<bool>();
    app.pending_tool_approval = Some(PendingToolApproval {
        command: "reboot".to_string(),
        approval_tx: Some(tx_query),
    });
    app.chat_input = "pourquoi veux-tu redémarrer ?".to_string();
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(
        app.pending_tool_approval.is_none(),
        "pending tool must be cleared"
    );
    assert!(
        !rx_query.await.expect("decline sent"),
        "Entering a question must decline pending command"
    );
}

#[test]
fn test_reasoning_edge_cases_thunk_and_partial_th() {
    use spiritty::agent::tools::{parse_tool_call, strip_think_blocks, ToolInvocation};

    // Case 1: Message 362 repro — DeepSeek emits </thunk> instead of </think> followed by DSML tool calls
    let msg_362 = "<think>Analysons la situation.\nIl faut modifier le fichier.\nAllons-y.</thunk>Modifions le main pour lire DEEPSEEK_API_KEY.\n\n<\u{ff5c}\u{ff5c}DSML\u{ff5c}\u{ff5c}tool_calls>\n<\u{ff5c}\u{ff5c}DSML\u{ff5c}\u{ff5c}invoke name=\"exec_command\">\n<\u{ff5c}\u{ff5c}DSML\u{ff5c}\u{ff5c}parameter name=\"cmd\" string=\"true\">sed -n '1,25p' /tmp/fetch.py</\u{ff5c}\u{ff5c}DSML\u{ff5c}\u{ff5c}parameter>\n</\u{ff5c}\u{ff5c}DSML\u{ff5c}\u{ff5c}invoke>\n</\u{ff5c}\u{ff5c}DSML\u{ff5c}\u{ff5c}tool_calls>";

    let stripped = strip_think_blocks(msg_362);
    assert!(!stripped.contains("<think>"));
    assert!(stripped.contains("sed -n '1,25p' /tmp/fetch.py"));

    let tool = parse_tool_call(msg_362);
    assert_eq!(
        tool,
        Some(ToolInvocation::RunCommand(
            "sed -n '1,25p' /tmp/fetch.py".to_string()
        ))
    );

    // Case 2: Message 350 repro — Trailing partial </th tag cut off at token limit
    let msg_350 = "<think>Voici ma réflexion.\nJe vais lancer ces commandes.\n\n</th";
    let stripped_350 = strip_think_blocks(msg_350);
    assert!(stripped_350.trim().is_empty());

    // Case 3: Direct transition from <think> to tool call without any closing tag
    let direct_transition = "<think>Je prépare la commande :\n<\u{ff5c}\u{ff5c}DSML\u{ff5c}\u{ff5c}invoke name=\"exec_command\">\n<\u{ff5c}\u{ff5c}DSML\u{ff5c}\u{ff5c}parameter name=\"command\" string=\"true\">uptime</\u{ff5c}\u{ff5c}DSML\u{ff5c}\u{ff5c}parameter>\n</\u{ff5c}\u{ff5c}DSML\u{ff5c}\u{ff5c}invoke>";
    let tool_direct = parse_tool_call(direct_transition);
    assert_eq!(
        tool_direct,
        Some(ToolInvocation::RunCommand("uptime".to_string()))
    );

    // Case 4: Real session repro — Model wrote malformed </thinking` followed by DSML and wrapped by trailing </think>
    let msg_391 = "<think>Enfin, `grep _cookieFile` dans le QML pour voir les usages.</thinking`plugin_settings.json` référence bien `deepseekWidget`.\n\n<\u{ff5c}\u{ff5c}DSML\u{ff5c}\u{ff5c}tool_calls>\n<\u{ff5c}\u{ff5c}DSML\u{ff5c}\u{ff5c}invoke name=\"exec_command\">\n<\u{ff5c}\u{ff5c}DSML\u{ff5c}\u{ff5c}parameter name=\"cmd\" string=\"true\">cd ~/.config/DankMaterialShell/plugins && ls</\u{ff5c}\u{ff5c}DSML\u{ff5c}\u{ff5c}parameter>\n</\u{ff5c}\u{ff5c}DSML\u{ff5c}\u{ff5c}invoke>\n</\u{ff5c}\u{ff5c}DSML\u{ff5c}\u{ff5c}tool_calls></think>";
    let tool_391 = parse_tool_call(msg_391);
    assert_eq!(
        tool_391,
        Some(ToolInvocation::RunCommand(
            "cd ~/.config/DankMaterialShell/plugins && ls".to_string()
        ))
    );
}

#[tokio::test]
async fn test_context_window_limit_detection() {
    use spiritty::app::App;
    let (event_tx, _event_rx) = tokio::sync::mpsc::unbounded_channel();
    let mut app = App::new(event_tx, 24, 80).unwrap();

    // DeepSeek family models must resolve to 131k (131_072)
    app.config.default_provider = spiritty::config::ProviderType::DeepSeek;
    if let Some(cfg) = app.config.providers.get_mut("deepseek") {
        cfg.model = "deepseek-flash".to_string();
    }
    assert_eq!(app.get_context_window_limit(), 131_072);

    if let Some(cfg) = app.config.providers.get_mut("deepseek") {
        cfg.model = "deepseek-v4-flash".to_string();
    }
    assert_eq!(app.get_context_window_limit(), 131_072);

    if let Some(cfg) = app.config.providers.get_mut("deepseek") {
        cfg.model = "deepseek-chat".to_string();
    }
    assert_eq!(app.get_context_window_limit(), 131_072);

    // Gemini models resolve to 1M (1_048_576)
    app.config.default_provider = spiritty::config::ProviderType::Gemini;
    if let Some(cfg) = app.config.providers.get_mut("gemini") {
        cfg.model = "gemini-3.8-flash".to_string();
    }
    assert_eq!(app.get_context_window_limit(), 1_048_576);

    // Claude models resolve to 200k (200_000)
    app.config.default_provider = spiritty::config::ProviderType::Anthropic;
    if let Some(cfg) = app.config.providers.get_mut("anthropic") {
        cfg.model = "claude-3-7-sonnet".to_string();
    }
    assert_eq!(app.get_context_window_limit(), 200_000);

    // Generic local fallback resolves to 8k (8_192)
    app.config.default_provider = spiritty::config::ProviderType::LmStudio;
    if let Some(cfg) = app.config.providers.get_mut("lmstudio") {
        cfg.model = "my-custom-local-model".to_string();
    }
    assert_eq!(app.get_context_window_limit(), 8_192);
}

#[test]
fn test_code_block_containing_fences_does_not_leak_proposals() {
    let snippet = r#"
Voici l'autopsie du bug :
```rust
while let Some(start_idx) = remaining.find("```") {
    let code = "test";
}
```
Pour ```tool:run_command```, il vérifie bien que la fence commence en début de ligne.
Mais dans `src/app.rs` :
```rust
while let Some(start_idx) = remaining.find("```") { ... }
```
De plus, si un bloc sans tag est capturé, il faut vérifier qu'il s'agit bien d'une commande.
"#;

    let proposals = spiritty::app::extract_all_command_proposals(snippet);
    assert!(
        proposals.is_empty(),
        "Expected no proposals from Rust code blocks and conversational text, got: {:?}",
        proposals
    );
    assert_eq!(spiritty::app::extract_command_proposal(snippet), None);
}

#[test]
fn test_repair_prematurely_closed_code_blocks_does_not_swallow_markdown() {
    use spiritty::app::extract_all_command_proposals;
    let input = "Voici un exemple vide :\n```bash\n\n```\nPour afficher les conteneurs, lancez :\n```bash\ndocker ps\n```\nEt voilà.";
    let proposals = extract_all_command_proposals(input);
    assert_eq!(proposals, vec!["docker ps"]);
}

#[test]
fn test_mouse_selection_reaches_bottom_prompt_line() {
    // In spiritty, prompt input occupies the bottom lines of chat_area up to area.bottom() - 1.
    // Ensure that the mouse selection clamping allows selecting the very last row (area.bottom() - 1),
    // such as the 3rd line of a 3-line multiline prompt.
    let panel_area = Rect {
        x: 0,
        y: 0,
        width: 80,
        height: 30,
    };

    let inner = Rect {
        x: panel_area.x,
        y: panel_area.y.saturating_add(1),
        width: panel_area.width,
        height: panel_area.height.saturating_sub(1),
    };

    let last_prompt_row = panel_area.bottom() - 1; // row 29
    let third_line_y = last_prompt_row;

    let s_y = third_line_y.clamp(inner.top(), inner.bottom().saturating_sub(1));
    let e_y = third_line_y.clamp(inner.top(), inner.bottom().saturating_sub(1));

    assert_eq!(
        s_y, 29,
        "Selection must reach the 3rd prompt line on row 29"
    );
    assert_eq!(
        e_y, 29,
        "Selection must reach the 3rd prompt line on row 29"
    );

    // Also verify when dragging past the panel area (e.g. into the footer divider at row 30)
    let dragged_past_y = panel_area.bottom() + 2; // row 32
    let clamped_e_y = dragged_past_y.clamp(inner.top(), inner.bottom().saturating_sub(1));
    assert_eq!(
        clamped_e_y, 29,
        "Dragging past panel must clamp to the bottom-most prompt line (29)"
    );
}

#[tokio::test]
async fn test_split_orientation_pty_sizing() {
    use spiritty::app::App;
    use spiritty::config::SplitOrientation;

    let (event_tx, _event_rx) = tokio::sync::mpsc::unbounded_channel();
    let mut app = App::new(event_tx, 40, 120).unwrap();

    // Horizontal (chat on top): the terminal keeps the full width and gets a reduced height.
    app.split_orientation = SplitOrientation::Horizontal;
    app.split_ratio = 70;
    let (h_rows, h_cols) = app.compute_pty_size(120, 40);
    assert_eq!(h_cols, 119);
    assert_eq!(h_rows, 11); // 37 workspace rows * (100 - 70) / 100

    // Vertical (side by side): the terminal keeps the full height and gets a reduced width.
    app.split_orientation = SplitOrientation::Vertical;
    app.split_ratio = 50;
    let (v_rows, v_cols) = app.compute_pty_size(120, 40);
    assert_eq!(v_rows, 37); // 40 - 3 (workspace height)
    assert_eq!(v_cols, 59); // (120 * 50 / 100) - 1
}

#[tokio::test]
async fn test_render_both_split_orientations() {
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
    use spiritty::app::App;
    use spiritty::config::SplitOrientation;

    let (event_tx, _event_rx) = tokio::sync::mpsc::unbounded_channel();
    let mut app = App::new(event_tx, 40, 120).unwrap();

    let backend = TestBackend::new(120, 40);
    let mut terminal = Terminal::new(backend).unwrap();

    // Horizontal: chat stacked above the terminal, sharing the workspace width.
    app.split_orientation = SplitOrientation::Horizontal;
    app.split_ratio = 70;
    terminal
        .draw(|f| spiritty::ui::draw(f, &mut app))
        .expect("horizontal draw must not panic");
    assert_eq!(app.chat_area.x, app.terminal_area.x);
    assert_eq!(app.chat_area.width, app.terminal_area.width);
    assert_eq!(app.chat_area.bottom(), app.terminal_area.top());
    assert!(app.chat_area.height > app.terminal_area.height);

    // Vertical: chat left of the terminal, sharing the workspace height.
    app.split_orientation = SplitOrientation::Vertical;
    app.split_ratio = 50;
    terminal
        .draw(|f| spiritty::ui::draw(f, &mut app))
        .expect("vertical draw must not panic");
    assert_eq!(app.chat_area.y, app.terminal_area.y);
    assert_eq!(app.chat_area.height, app.terminal_area.height);
    assert_eq!(app.chat_area.right(), app.terminal_area.left());
    assert!(app.chat_area.left() < app.terminal_area.left());
}

#[tokio::test]
async fn test_render_swapped_panels() {
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
    use spiritty::app::App;
    use spiritty::config::SplitOrientation;

    let (event_tx, _event_rx) = tokio::sync::mpsc::unbounded_channel();
    let mut app = App::new(event_tx, 40, 120).unwrap();

    let backend = TestBackend::new(120, 40);
    let mut terminal = Terminal::new(backend).unwrap();

    // Vertical swapped: terminal left, chat right.
    app.split_orientation = SplitOrientation::Vertical;
    app.split_ratio = 50;
    app.split_swapped = true;
    terminal
        .draw(|f| spiritty::ui::draw(f, &mut app))
        .expect("swapped vertical draw must not panic");
    assert_eq!(app.chat_area.y, app.terminal_area.y);
    assert_eq!(app.chat_area.height, app.terminal_area.height);
    assert_eq!(app.terminal_area.right(), app.chat_area.left());
    assert!(app.terminal_area.left() < app.chat_area.left());

    // Horizontal swapped: terminal top, chat bottom.
    app.split_orientation = SplitOrientation::Horizontal;
    app.split_ratio = 70;
    app.split_swapped = true;
    terminal
        .draw(|f| spiritty::ui::draw(f, &mut app))
        .expect("swapped horizontal draw must not panic");
    assert_eq!(app.chat_area.x, app.terminal_area.x);
    assert_eq!(app.chat_area.width, app.terminal_area.width);
    assert_eq!(app.terminal_area.bottom(), app.chat_area.top());
    assert!(app.chat_area.height > app.terminal_area.height);

    // Restore: not swapped puts the chat back first.
    app.split_swapped = false;
    terminal
        .draw(|f| spiritty::ui::draw(f, &mut app))
        .expect("unswapped draw must not panic");
    assert_eq!(app.chat_area.bottom(), app.terminal_area.top());
}

#[test]
fn test_help_modal_single_column_and_scroll() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
    use spiritty::i18n::Language;
    use spiritty::ui::components::{HelpModal, HelpModalState};

    let mut state = HelpModalState::new();
    let key = |c| KeyEvent::new(c, KeyModifiers::NONE);

    // max_scroll is only known after a render, so scrolling is clamped to 0 initially.
    assert!(!HelpModal::handle_key(key(KeyCode::Down), &mut state));
    assert_eq!(state.scroll, 0);

    // Render on a short terminal: the content overflows and becomes scrollable.
    let mut term = Terminal::new(TestBackend::new(80, 20)).unwrap();
    term.draw(|f| {
        HelpModal::render_modal(f.area(), f.buffer_mut(), Language::Fr, &state);
    })
    .unwrap();

    for _ in 0..5 {
        HelpModal::handle_key(key(KeyCode::Down), &mut state);
    }
    assert_eq!(state.scroll, 5);

    HelpModal::handle_key(key(KeyCode::End), &mut state);
    assert!(state.scroll > 0, "End should jump to the bottom");
    HelpModal::handle_key(key(KeyCode::Home), &mut state);
    assert_eq!(state.scroll, 0, "Home should jump back to the top");

    // Re-render at the bottom edge to make sure slicing never panics.
    HelpModal::handle_key(key(KeyCode::End), &mut state);
    term.draw(|f| {
        HelpModal::render_modal(f.area(), f.buffer_mut(), Language::En, &state);
    })
    .unwrap();

    // Esc closes the modal.
    assert!(HelpModal::handle_key(key(KeyCode::Esc), &mut state));
}

#[test]
fn test_command_recovered_after_unclosed_think() {
    use spiritty::app::extract_all_command_proposals;

    // Real-session repro: the model opened `<think>` and never closed it, then emitted its real
    // command as a ```bash block. That block must be extracted, not swallowed as reasoning.
    let text = "<think>Je réfléchis encore, et je n'ai pas fermé ma balise.\n```bash\nuptime && df -h\n```";
    let proposals = extract_all_command_proposals(text);
    assert_eq!(proposals.len(), 1, "got {proposals:?}");
    assert_eq!(proposals[0], "uptime && df -h");
}

#[test]
fn test_command_recovered_when_closing_fence_glued_to_think_tag() {
    use spiritty::app::extract_all_command_proposals;

    // Real-session repro: a stray `</think>` was glued to the closing code fence, which made
    // `find_closing_code_fence` reject the block and silently drop the command.
    let text =
        "<think>Analyse.\n</think>Je lance :\n```bash\nsudo systemctl restart nginx\n```</think>";
    let proposals = extract_all_command_proposals(text);
    assert_eq!(proposals.len(), 1, "got {proposals:?}");
    assert_eq!(proposals[0], "sudo systemctl restart nginx");
}

#[test]
fn test_recover_command_from_reasoning_guards_on_visible_answer() {
    use spiritty::app::recover_command_from_reasoning;

    // Dead turn: the model put its command inside the reasoning and produced no visible answer.
    let dead = "<think>Je réfléchis encore.\n```bash\nsudo systemctl restart nginx\n```</think>";
    assert_eq!(
        recover_command_from_reasoning(dead),
        Some("sudo systemctl restart nginx".to_string())
    );

    // There IS a visible answer: a command merely *considered* in the reasoning must be ignored.
    let alive = "<think>peut-être :\n```bash\nrm -rf /\n```</think>Voici mon analyse.";
    assert_eq!(recover_command_from_reasoning(alive), None);
}

#[tokio::test]
async fn test_dead_turn_recovery_promotes_reasoning_command() {
    use spiritty::app::{App, ChatMessage, MessageRole};

    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
    let mut app = App::new(tx, 24, 80).unwrap();
    // Disable auto-approval so the recovered (Safe) command is not injected into the PTY,
    // keeping the assertion focused on the recovered proposal itself.
    app.config.auto_approve = spiritty::config::AutoApproveLevel::Off;
    app.messages.clear();
    app.messages.push(ChatMessage {
        role: MessageRole::Assistant,
        content: "<think>Analyse.\n```bash\nuptime && df -h\n```</think>".to_string(),
        command_proposal: None,
        attachments: Vec::new(),
    });

    app.on_agent_done();

    let last = app.messages.last().expect("assistant message kept");
    assert!(
        last.content.contains("```bash\nuptime && df -h\n```"),
        "recovered command must be promoted to a visible block: {}",
        last.content
    );
    assert_eq!(last.command_proposal.as_deref(), Some("uptime && df -h"));
    assert_eq!(
        app.all_command_proposals(),
        vec!["uptime && df -h".to_string()],
        "the recovered command must be actionable"
    );
}

#[tokio::test]
async fn test_horizontal_split_prompt_wrapping_not_overwritten_by_divider() {
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
    use spiritty::app::App;
    use spiritty::config::SplitOrientation;

    let (event_tx, _event_rx) = tokio::sync::mpsc::unbounded_channel();
    let mut app = App::new(event_tx, 40, 80).unwrap();

    app.split_orientation = SplitOrientation::Horizontal;
    app.split_ratio = 50;
    app.chat_input = "x".repeat(100);
    app.cursor_pos = app.chat_input.len();

    let backend = TestBackend::new(80, 40);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal
        .draw(|f| {
            spiritty::ui::draw(f, &mut app);
        })
        .expect("horizontal draw must not panic");

    let pos = terminal.get_cursor_position().unwrap();
    let (cx, cy) = (pos.x, pos.y);
    let cell = terminal.backend().buffer().cell((cx, cy)).unwrap();
    assert_ne!(
        cell.symbol(),
        "─",
        "cursor is sitting on the divider '─' at ({}, {})",
        cx,
        cy
    );
}

#[tokio::test]
async fn test_natural_approval_execution_lifecycle() {
    use spiritty::app::{App, ChatMessage, MessageRole};

    let (event_tx, _event_rx) = tokio::sync::mpsc::unbounded_channel();
    let mut app = App::new(event_tx, 24, 80).expect("create app");

    // 1. Assistant proposes a command
    app.messages.push(ChatMessage {
        role: MessageRole::Assistant,
        content: "Voici la commande :\n```bash\nsudo mkdir -p /mnt/data\n```".to_string(),
        command_proposal: Some("sudo mkdir -p /mnt/data".to_string()),
        attachments: Vec::new(),
    });

    assert_eq!(
        app.all_command_proposals(),
        vec!["sudo mkdir -p /mnt/data".to_string()]
    );

    // 2. User types "?" -> executes the command!
    app.chat_input = "?".to_string();
    app.submit_chat_input();

    assert!(
        app.active_pty_tool.is_some(),
        "Typing '?' must execute the proposed command"
    );
    let last_user_msg = app.messages.iter().rev().find(|m| m.role == MessageRole::User).unwrap();
    assert_eq!(last_user_msg.content, "💻 `sudo mkdir -p /mnt/data`");

    // Clear active pty tool to simulate completed execution
    app.active_pty_tool = None;
    app.agent.is_generating = false;

    // Simulate completion result in history
    app.messages.push(ChatMessage {
        role: MessageRole::User,
        content: "[RÉSULTAT DE L'EXÉCUTION DE LA COMMANDE 'sudo mkdir -p /mnt/data']:\nOK".to_string(),
        command_proposal: None,
        attachments: Vec::new(),
    });
    app.messages.push(ChatMessage {
        role: MessageRole::Assistant,
        content: "Le point de montage est prêt.".to_string(),
        command_proposal: None,
        attachments: Vec::new(),
    });

    // 3. User says "ok" after completion -> must NOT re-execute already completed command
    assert!(
        app.all_command_proposals().is_empty(),
        "Finished proposals must not be re-executed when acknowledging completion"
    );
    app.chat_input = "ok".to_string();
    app.submit_chat_input();
    assert!(
        app.active_pty_tool.is_none(),
        "Must not trigger re-execution of finished command"
    );

    // 4. Test pending tool approval with "?" directly approving
    let (approval_tx, mut approval_rx) = tokio::sync::oneshot::channel::<bool>();
    app.on_agent_tool_request("systemctl restart nginx".to_string(), approval_tx);
    assert!(app.pending_tool_approval.is_some());

    // Fulfill via execute_command_by_index directly
    assert!(app.execute_command_by_index(0, true));
    assert!(app.pending_tool_approval.is_none());
    assert_eq!(approval_rx.try_recv().unwrap(), true);
}

#[tokio::test]
async fn test_pinned_prompt_header_when_scrolled() {
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
    use spiritty::app::{App, ChatMessage, MessageRole};

    let (event_tx, _event_rx) = tokio::sync::mpsc::unbounded_channel();
    let mut app = App::new(event_tx, 25, 80).expect("create app");

    // Add user message
    app.messages.push(ChatMessage {
        role: MessageRole::User,
        content: "installer nginx et php-fpm sur debian".to_string(),
        command_proposal: None,
        attachments: Vec::new(),
    });

    // Add a very long assistant message that will force scrolling
    let long_response = (1..=30)
        .map(|i| format!("Ligne de réponse numéro {} avec des détails techniques.", i))
        .collect::<Vec<_>>()
        .join("\n");

    app.messages.push(ChatMessage {
        role: MessageRole::Assistant,
        content: long_response,
        command_proposal: None,
        attachments: Vec::new(),
    });

    // Chat is scrolled to the bottom (chat_scroll_from_bottom == 0)
    app.chat_scroll_from_bottom = 0;

    let backend = TestBackend::new(80, 25);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal
        .draw(|f| {
            spiritty::ui::draw(f, &mut app);
        })
        .expect("draw must succeed");

    // Pinned header must be active because the user message scrolled above the viewport
    assert!(
        app.chat_pinned_hit.borrow().is_some(),
        "chat_pinned_hit must be populated when user prompt is scrolled off-screen"
    );

    // Verify rendered buffer contains the pinned indicator and text
    let buffer = terminal.backend().buffer();
    let content: String = (0..buffer.area.height)
        .map(|y| {
            (0..buffer.area.width)
                .map(|x| buffer.cell((x, y)).unwrap().symbol().to_string())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n");

    assert!(
        content.contains("📌") || content.contains("Vous") || content.contains("You"),
        "Buffer should contain pinned prompt icon/prefix"
    );
    assert!(
        content.contains("installer nginx"),
        "Buffer should contain pinned prompt text"
    );
    assert!(
        content.contains("[↑]"),
        "Buffer should contain pinned hint badge [↑]"
    );

    // Now click the pinned header to jump back to the prompt
    let (pinned_rect, _u_idx, _content_top) = app.chat_pinned_hit.borrow().unwrap();
    let click_x = pinned_rect.x + 2;
    let click_y = pinned_rect.y;

    use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
    app.handle_mouse(
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: click_x,
            row: click_y,
            modifiers: crossterm::event::KeyModifiers::NONE,
        },
        80,
    );

    // Redraw: after jumping to the prompt, the prompt is in the viewport so pinned header disappears
    terminal
        .draw(|f| {
            spiritty::ui::draw(f, &mut app);
        })
        .expect("draw must succeed");

    assert!(
        app.chat_pinned_hit.borrow().is_none(),
        "chat_pinned_hit must disappear once the prompt is visible on screen"
    );

    // 2. Test that intermediate tool execution captures [RÉSULTAT...] are never pinned!
    // Scroll back down
    app.chat_scroll_from_bottom = 0;
    // Add intermediate tool result message (like tool:run_command execution)
    app.messages.push(ChatMessage {
        role: MessageRole::User,
        content: "[RÉSULTAT DE L'EXÉCUTION DE LA COMMANDE 'echo \"=== REGISTRY INFO ===\"']: OK".to_string(),
        command_proposal: None,
        attachments: Vec::new(),
    });
    // Add assistant reply analyzing the tool result
    let long_analysis = (1..=20)
        .map(|i| format!("Analyse du résultat ligne {}.", i))
        .collect::<Vec<_>>()
        .join("\n");
    app.messages.push(ChatMessage {
        role: MessageRole::Assistant,
        content: long_analysis,
        command_proposal: None,
        attachments: Vec::new(),
    });

    terminal
        .draw(|f| {
            spiritty::ui::draw(f, &mut app);
        })
        .expect("draw must succeed");

    assert!(
        app.chat_pinned_hit.borrow().is_some(),
        "chat_pinned_hit must be active"
    );

    let buffer2 = terminal.backend().buffer();
    let content2: String = (0..buffer2.area.height)
        .map(|y| {
            (0..buffer2.area.width)
                .map(|x| buffer2.cell((x, y)).unwrap().symbol().to_string())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n");

    // Must still pin the real user prompt, NOT the raw [RÉSULTAT...] internal block
    assert!(
        content2.contains("installer nginx"),
        "Buffer should pin the original user prompt, found:\n{}",
        content2
    );
    assert!(
        !content2.contains("RÉSULTAT"),
        "Pinned prompt header must NEVER pin raw [RÉSULTAT...] capture blocks!"
    );
}

#[tokio::test]
async fn test_dynamic_prompt_height_and_scrolling() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
    use spiritty::app::App;
    use spiritty::config::SplitOrientation;

    let (event_tx, _event_rx) = tokio::sync::mpsc::unbounded_channel();
    let mut app = App::new(event_tx, 30, 80).unwrap();
    app.focus = spiritty::app::Focus::Chat;
    app.split_orientation = SplitOrientation::Vertical;
    app.split_ratio = 50;

    let backend = TestBackend::new(80, 30);
    let mut terminal = Terminal::new(backend).unwrap();

    // 1. Empty prompt -> 1 line height.
    terminal
        .draw(|f| {
            spiritty::ui::draw(f, &mut app);
        })
        .unwrap();
    let pos_empty = terminal.get_cursor_position().unwrap();

    // Single line prompt -> still 1 line height, cursor y identical
    app.chat_input = "Hello world".to_string();
    app.cursor_pos = app.chat_input.len();
    terminal
        .draw(|f| {
            spiritty::ui::draw(f, &mut app);
        })
        .unwrap();
    let pos_1line = terminal.get_cursor_position().unwrap();
    assert_eq!(
        pos_empty.y, pos_1line.y,
        "1-line prompt should have same bottom y as empty prompt"
    );

    // 2. 2-line prompt -> expands to 2 lines. Cursor is on line 2, so cursor.y is pos_1line.y,
    // while line 1 is rendered at pos_1line.y - 1.
    app.chat_input = "Line 1\nLine 2".to_string();
    app.cursor_pos = app.chat_input.len();
    terminal
        .draw(|f| {
            spiritty::ui::draw(f, &mut app);
        })
        .unwrap();
    let pos_2lines = terminal.get_cursor_position().unwrap();
    assert_eq!(
        pos_2lines.y, pos_1line.y,
        "Cursor on 2nd line should be on the bottom row"
    );

    // Move cursor up to Line 1
    app.handle_key(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE));
    terminal
        .draw(|f| {
            spiritty::ui::draw(f, &mut app);
        })
        .unwrap();
    let pos_line1 = terminal.get_cursor_position().unwrap();
    assert_eq!(
        pos_line1.y,
        pos_1line.y - 1,
        "Cursor on 1st line should be 1 row above bottom row"
    );

    // 3. 4-line prompt -> capped at 2 lines height, scrolls!
    app.chat_input = "Line 1\nLine 2\nLine 3\nLine 4".to_string();
    app.cursor_pos = app.chat_input.len();
    terminal
        .draw(|f| {
            spiritty::ui::draw(f, &mut app);
        })
        .unwrap();
    let pos_4lines = terminal.get_cursor_position().unwrap();
    assert_eq!(
        pos_4lines.y, pos_1line.y,
        "Cursor on 4th line remains on bottom row of 2-line input"
    );
    assert_eq!(
        app.chat_input_scroll.get(),
        2,
        "Scroll should be 2 to show Line 3 and Line 4"
    );

    // Move cursor up to Line 3: Line 3 was already visible (on top row of the 2-line box)
    app.handle_key(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE));
    terminal
        .draw(|f| {
            spiritty::ui::draw(f, &mut app);
        })
        .unwrap();
    let pos_line3 = terminal.get_cursor_position().unwrap();
    assert_eq!(
        pos_line3.y,
        pos_1line.y - 1,
        "Cursor moves to top row of 2-line box without jumping scroll"
    );
    assert_eq!(app.chat_input_scroll.get(), 2, "Scroll remains 2");

    // Move cursor up to Line 2: scrolls up by 1
    app.handle_key(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE));
    terminal
        .draw(|f| {
            spiritty::ui::draw(f, &mut app);
        })
        .unwrap();
    assert_eq!(
        app.chat_input_scroll.get(),
        1,
        "Scroll becomes 1 to show Line 2 and Line 3"
    );

    // Move cursor up to Line 1: scrolls up by 1
    app.handle_key(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE));
    terminal
        .draw(|f| {
            spiritty::ui::draw(f, &mut app);
        })
        .unwrap();
    assert_eq!(
        app.chat_input_scroll.get(),
        0,
        "Scroll becomes 0 to show Line 1 and Line 2"
    );
}

