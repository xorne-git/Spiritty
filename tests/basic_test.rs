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
    assert_eq!(parse_command_execution_request("ok", 0), None);
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

    let glitched = "Je corrige avec le bon pattern :\n\n```bash\n \n```\nfor v in 7.4 8.4 8.5; do\n  sudo sed -i -e 's/a/b/' /etc/php/$v/fpm/php.ini\ndone\n\nsudo systemctl restart php-fpm`";
    let repaired = repair_prematurely_closed_code_blocks(glitched);

    assert!(repaired.contains("```bash\nfor v in 7.4 8.4 8.5; do"));
    assert!(repaired.ends_with("sudo systemctl restart php-fpm\n```"));

    let proposals = extract_all_command_proposals(glitched);
    assert_eq!(proposals.len(), 1);
    assert!(proposals[0].starts_with("for v in 7.4 8.4 8.5; do"));
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

        // 1. Left info (Provider & Model) must always be present (full priority)
        assert!(
            footer_text.contains("󰚩")
                || footer_text.contains("Ollama")
                || footer_text.contains("DeepSeek"),
            "Width {} should contain provider/model info! Rendered: '{}'",
            width,
            footer_text
        );

        // 2. Right shortcuts: F1 and Ctrl+P / ^P are prioritized
        assert!(
            footer_text.contains("F1"),
            "Width {} should contain F1 shortcut! Rendered: '{}'",
            width,
            footer_text
        );
        assert!(
            footer_text.contains("P") || footer_text.contains("Config"),
            "Width {} should contain P/Config shortcut! Rendered: '{}'",
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
            assert!(
                footer_text.contains("Hosts") || footer_text.contains("B"),
                "Width 140 should contain Hosts! Rendered: '{}'",
                footer_text
            );
            assert!(
                footer_text.contains("MCP") || footer_text.contains("M"),
                "Width 140 should contain MCP! Rendered: '{}'",
                footer_text
            );
            assert!(
                footer_text.contains("Sessions") || footer_text.contains("H"),
                "Width 140 should contain Sessions! Rendered: '{}'",
                footer_text
            );
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
