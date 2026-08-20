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
        let has_content = (0..width).any(|x| buf.cell((x, y)).map(|c| c.symbol() != " ").unwrap_or(false));
        if has_content {
            rendered_lines = (y + 1) as u16;
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
    println!("Ratatui rendered: {}, Calculated: {}", rendered_lines, calculated);
    assert_eq!(rendered_lines, calculated);

    // Test 2: Complex multi-line conversation with code blocks, list items, and short lines
    let complex_lines = vec![
        Line::from("👤 Bonjour, peux-tu m'aider ?"),
        Line::from(""),
        Line::from("👻 Oui bien sûr ! Voici ce que nous allons vérifier :"),
        Line::from("  - Point 1 : Vérifier le statut du service avec systemctl"),
        Line::from("  - Point 2 : Vérifier les journaux avec journalctl -xeu dms.service"),
        Line::from(""),
        Line::from("⚡ COMMANDE #1 Safe"),
        Line::from("  systemctl --user status dms.service"),
        Line::from("  ↵ Enter Exécuter"),
        Line::from(""),
        Line::from("💻 `systemctl --user status dms.service`"),
        Line::from(""),
        Line::from("👻 Analyse terminée avec succès. Tout fonctionne parfaitement."),
    ];

    let mut buf2 = Buffer::empty(Rect::new(0, 0, width, 100));
    let p2 = Paragraph::new(complex_lines.clone()).wrap(Wrap { trim: false });
    p2.render(Rect::new(0, 0, width, 100), &mut buf2);

    let mut rendered_lines2: u16 = 0;
    for y in 0..100 {
        let has_content = (0..width).any(|x| buf2.cell((x, y)).map(|c| c.symbol() != " ").unwrap_or(false));
        if has_content {
            rendered_lines2 = (y + 1) as u16;
        }
    }

    let calculated2 = count_lines(&complex_lines, width);
    println!("Ratatui rendered complex: {}, Calculated: {}", rendered_lines2, calculated2);
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
            rendered_lines3 = (y + 1) as u16;
            println!("Row {:02}: |{}|", y, line_str);
        }
    }

    for (idx, l) in table_lines.iter().enumerate() {
        let c = count_lines(&[l.clone()], width);
        println!("Line {:02} (count {}): {}", idx, c, l);
    }

    let calculated3 = count_lines(&table_lines, width);
    println!("Ratatui rendered table: {}, Calculated: {}", rendered_lines3, calculated3);
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
            rendered_lines4 = (y + 1) as u16;
            println!("Callout row {:02}: |{}|", y, line_str);
        }
    }

    let calculated4 = count_lines(&callout_lines, width);
    println!("Ratatui rendered callout: {}, Calculated: {}", rendered_lines4, calculated4);
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
    });

    app.messages.push(ChatMessage {
        role: MessageRole::User,
        content: "Génère-moi un fichier ~/audit_systeme.md récapitulant les informations clés de mon noyau, shell et mémoire RAM.".to_string(),
        command_proposal: None,
    });

    app.messages.push(ChatMessage {
        role: MessageRole::Assistant,
        content: "<think>\nL'utilisateur veut que je génère un fichier ~/audit_systeme.md avec des infos sur le noyau, le shell et la RAM. Je dois d'abord récupérer ces informations via tool:run_command, puis créer le fichier.\n\nJe vais exécuter les commandes nécessaires pour obtenir :\n- Version du noyau (uname -r)\n- Shell en cours (echo $SHELL ou whoami)\n- Informations sur la RAM (free -h)\n</think>\n```tool:run_command\nuname -r && echo \"---\" && whoami && echo \"---\" && free -h | head -2 && echo \"---\" && cat /etc/os-release | grep PRETTY_NAME\n```".to_string(),
        command_proposal: None,
    });

    app.messages.push(ChatMessage {
        role: MessageRole::User,
        content: "💻 `uname -r && echo \"---\" && whoami && echo \"---\" && free -h | head -2 && echo \"---\" && cat /etc/os-release | grep PRETTY_NAME`".to_string(),
        command_proposal: None,
    });

    app.messages.push(ChatMessage {
        role: MessageRole::Assistant,
        content: "```bash\ncat > ~/audit_systeme.md << 'EOF'\n# Audit Système - CachyOS\n\n## Informations Clés\n\n### Noyau (Kernel)\n- **Version** : 7.1.8-1-cachyos\n- **Distribution** : CachyOS\n\n### Shell\n- **Shell actif** : xorne (alias pour fish)\n- **Utilisateur** : xorne\n\n### Mémoire RAM\n| Statut | Quantité |\n|--------|----------|\n| **Total** | 31 GiB |\n| **Utilisé** | 11 GiB |\n| **Libre** | 885 MiB |\n| **Disponible** | 19 GiB |\n\n### Résumé\n- **Utilisation RAM** : ~36% (11 GiB / 31 GiB)\n- **Etat** : ✅ Bon - La mémoire est correctement gérée avec suffisamment de ressources disponibles.\nEOF\n```".to_string(),
        command_proposal: None,
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
        rendered_text.contains("Alt") && rendered_text.contains("Exécuter"),
        "The bottom action buttons [Alt 1 Exécuter] must be visible in the chat viewport!"
    );
    assert!(
        rendered_text.contains("EOF"),
        "The command content EOF must be visible in the chat viewport!"
    );
}

#[tokio::test]
async fn test_chat_overscroll_down() {
    use spiritty::app::App;
    let (event_tx, _event_rx) = tokio::sync::mpsc::unbounded_channel();
    let mut app = App::new(event_tx, 55, 100).expect("create app");

    assert_eq!(app.chat_scroll_from_bottom, 0);
    assert_eq!(app.chat_scroll_extra_down, 0);

    // Scrolling down while at the bottom initiates overscroll
    app.scroll_chat_down(3);
    assert_eq!(app.chat_scroll_extra_down, 3);
    assert_eq!(app.chat_scroll_from_bottom, 0);

    app.scroll_chat_down(5);
    assert_eq!(app.chat_scroll_extra_down, 8);

    // Scrolling up consumes overscroll first before scrolling into history
    app.scroll_chat_up(4);
    assert_eq!(app.chat_scroll_extra_down, 4);
    assert_eq!(app.chat_scroll_from_bottom, 0);

    app.scroll_chat_up(6);
    assert_eq!(app.chat_scroll_extra_down, 0);
    assert_eq!(app.chat_scroll_from_bottom, 2);

    // Resetting clears both
    app.reset_chat_scroll();
    assert_eq!(app.chat_scroll_from_bottom, 0);
    assert_eq!(app.chat_scroll_extra_down, 0);
}

#[tokio::test]
async fn test_empty_chat_badge_shows_zero() {
    use spiritty::app::App;
    use spiritty::ui::chat_panel::ChatPanel;
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;

    let (event_tx, _event_rx) = tokio::sync::mpsc::unbounded_channel();
    let app = App::new(event_tx, 55, 100).expect("create app");
    let mut buf = Buffer::empty(Rect::new(0, 0, 100, 50));

    let panel = ChatPanel::new(&app);
    panel.render_panel(Rect::new(0, 0, 100, 50), &mut buf);

    let top_header = (0..100)
        .map(|x| buf.cell((x, 0)).map(|c| c.symbol()).unwrap_or(" "))
        .collect::<String>();

    assert!(top_header.contains("0 l."), "Empty chat panel header must show 0 l. instead of 4 l. (got: {})", top_header);
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

    let bad_cmd = "echo \"=== SERVICES ===\" && \\\nsystemctl list-units --type=service && \\\necho \"done\"";
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
    assert!(cleaned_heredoc.contains("# Audit Système - CachyOS"), "Markdown headings starting with # must not be stripped in heredocs");
    assert!(cleaned_heredoc.contains("<< 'EOF'"));
    assert!(!cleaned_heredoc.contains("&& #"), "Heredoc body lines must not be joined with &&");
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
    });

    app.agent.is_generating = true;
    assert!(app.agent.is_generating);

    app.stop_agent_generation();

    assert!(!app.agent.is_generating);
    assert_eq!(app.messages.last().unwrap().content, "Génération partielle...");
    assert!(app.toast_message.is_some());

    let _ = spiritty::session::SessionStorage::delete(&app.current_session.id);
}

#[test]
fn test_format_command_for_pty() {
    use spiritty::app::{format_command_for_pty, format_command_for_pty_with_session};

    // 1. Simple cd command
    assert_eq!(format_command_for_pty("cd /var/log", "fish"), " cd /var/log\n");

    // 2. Simple single line in local fish (wraps in bash -c with space)
    assert_eq!(format_command_for_pty("free -h", "fish"), " bash -c 'free -h'\n");

    // 3. Simple single line in local bash (no bash -c wrapping needed)
    assert_eq!(format_command_for_pty("free -h", "bash"), " free -h\n");

    // 4. Simple single line on remote SSH (tool capture -> pure clean command)
    let remote_tool_cmd = format_command_for_pty_with_session("free -h", "fish", true, true);
    assert_eq!(remote_tool_cmd, " free -h\n");

    // 5. Simple single line on remote SSH (manual user Alt+1 -> pure clean command)
    let remote_user_cmd = format_command_for_pty_with_session("cat ~/audit_systeme.md", "fish", true, false);
    assert_eq!(remote_user_cmd, " cat ~/audit_systeme.md\n");

    // 6. Local multiline heredoc script -> runs clean verbose temporary script (with leading space to avoid history)
    let multiline_heredoc = "cat > ~/audit.md << EOF\n# Title\nEOF";
    let formatted_local = format_command_for_pty(multiline_heredoc, "fish");
    assert!(formatted_local.starts_with(" bash -v "));
    assert!(formatted_local.ends_with("spiritty_exec.sh\n"));

    // 7. Remote SSH multiline heredoc script -> single line base64 pipe
    let formatted_remote = format_command_for_pty_with_session(multiline_heredoc, "fish", true, true);
    assert!(formatted_remote.starts_with(" echo '"));
    assert!(formatted_remote.ends_with("' | base64 -d | bash -v\n"));
    assert_eq!(formatted_remote.matches('\n').count(), 1);
}

#[test]
fn test_repair_missing_heredoc_terminator() {
    use spiritty::app::repair_missing_heredoc_terminator;

    // 1. Missing EOF
    let truncated = "cat > ~/audit_systeme.md << 'EOF'\nAudit Système - CachyOS\nDate : 2026-08-20";
    let repaired = repair_missing_heredoc_terminator(truncated);
    assert_eq!(repaired, "cat > ~/audit_systeme.md << 'EOF'\nAudit Système - CachyOS\nDate : 2026-08-20\nEOF\n");

    // 2. Missing ENDOFFILE
    let truncated2 = "cat << 'ENDOFFILE' > ~/file.txt\nSome content";
    let repaired2 = repair_missing_heredoc_terminator(truncated2);
    assert_eq!(repaired2, "cat << 'ENDOFFILE' > ~/file.txt\nSome content\nENDOFFILE\n");

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
fn test_parse_command_execution_request() {
    use spiritty::app::parse_command_execution_request;

    // Direct affirmative phrases
    assert_eq!(parse_command_execution_request("ok", 3), Some(0));
    assert_eq!(parse_command_execution_request("oui", 3), Some(0));
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
    assert_eq!(parse_command_execution_request("comment installer nginx ?", 3), None);
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

                if !cursor_found && cursor_byte_pos >= byte_offset && cursor_byte_pos <= byte_offset + word_bytes {
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
    println!("Prompt end cursor: row {}, col {}, total rows {}", c_row, c_col, t_rows);

    let (c_row2, c_col2, t_rows2) = compute_prompt_cursor_and_lines(text, text.len() - 1, width);
    println!("Prompt after backspace cursor: row {}, col {}, total rows {}", c_row2, c_col2, t_rows2);

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



