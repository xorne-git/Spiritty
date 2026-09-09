use anyhow::Result;
use crossterm::{
    cursor::{EnableBlinking, SetCursorStyle},
    event::{DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use spiritty::{
    app::App,
    event::{AppEvent, EventHandler},
    ui,
};
use std::{
    io::{self, stdout},
    panic, thread,
    time::Duration,
};

#[tokio::main]
async fn main() -> Result<()> {
    // Parse CLI options before initializing raw terminal mode
    let cli = spiritty::cli::CliOptions::parse_from_args(std::env::args());

    if cli.show_version {
        spiritty::cli::CliOptions::print_version();
        return Ok(());
    }

    if cli.show_help {
        spiritty::cli::CliOptions::print_help();
        return Ok(());
    }

    if cli.list_sessions {
        spiritty::cli::CliOptions::print_sessions_list()?;
        return Ok(());
    }

    // Setup panic hook to always restore terminal state on panic
    setup_panic_hook();

    // Restore the terminal and exit cleanly on external termination signals too: an
    // externally killed TUI (SIGTERM from `kill`, SIGHUP from a closed window) used to
    // leave the tty in raw mode with no cursor and stale screen content — the user then
    // needed a manual `reset`. SIGKILL cannot be caught (only a manual `reset` helps).
    setup_termination_signal_hook();

    // Initialize raw terminal, bracketed paste, mouse capture and alternate screen
    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(
        stdout,
        EnterAlternateScreen,
        EnableBracketedPaste,
        EnableMouseCapture,
        EnableBlinking,
        SetCursorStyle::DefaultUserShape
    )?;

    // Enable Kitty/Disambiguate keyboard protocol for Shift+Enter and modifier distinction
    let _ = execute!(
        stdout,
        crossterm::event::PushKeyboardEnhancementFlags(
            crossterm::event::KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
        )
    );

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Calculate initial split dimensions for the PTY (full screen height)
    let term_size = terminal.size()?;
    let initial_config = spiritty::config::Config::load();
    let split_ratio = initial_config.get_split_ratio();
    let initial_rows = term_size.height.saturating_sub(3).max(1);
    let initial_cols = (((term_size.width as u32 * (100 - split_ratio as u32)) / 100) as u16)
        .saturating_sub(1)
        .max(1);

    // Create event loop and application state
    let tick_rate = Duration::from_millis(90); // ~11 FPS for smooth, balanced spinner cadence
    let mut event_handler = EventHandler::new(tick_rate);
    let mut app = App::new(event_handler.sender(), initial_rows, initial_cols)?;
    app.debug = cli.debug;

    // Silent multi-provider pricing refresh at startup (updates the local cache; no toast)
    app.trigger_pricing_update_announced(false);

    // Apply CLI session resumption or overrides
    if cli.continue_last_session {
        if let Ok(sessions) = spiritty::session::SessionStorage::list_sessions() {
            if let Some(last) = sessions.first() {
                app.load_session(&last.id);
            }
        }
    } else if let Some(ref session_id) = cli.session_id {
        app.load_session(session_id);
    }

    app.apply_cli_overrides(cli.provider, cli.model, cli.auto_approve, cli.ssh_target);

    if let Some(ref prompt) = cli.initial_prompt {
        app.submit_initial_prompt(prompt);
    }

    // Main event loop
    let res = run_loop(&mut terminal, &mut app, &mut event_handler).await;

    // Restore terminal state cleanly
    let _ = execute!(terminal.backend_mut(), SetCursorStyle::DefaultUserShape);
    let _ = execute!(
        terminal.backend_mut(),
        crossterm::event::PopKeyboardEnhancementFlags
    );
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableBracketedPaste,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        eprintln!("Application error: {:?}", err);
    }

    Ok(())
}

async fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
    event_handler: &mut EventHandler,
) -> Result<()> {
    // Initial frame render
    terminal.draw(|f| {
        ui::draw(f, app);
    })?;

    while !app.should_quit {
        // Wait for next event
        if let Some(event) = event_handler.next().await {
            let mut should_render = true;

            match event {
                AppEvent::Key(key) => app.handle_key(key),
                AppEvent::Paste(text) => app.handle_paste(text),
                AppEvent::PasteInto(text) => app.paste_into(text),
                AppEvent::PasteImage(image) => app.handle_paste_image(image),
                AppEvent::Mouse(mouse) => {
                    let total_width = terminal.size()?.width;
                    app.handle_mouse(mouse, total_width);
                }
                AppEvent::Resize(w, h) => {
                    terminal.autoresize()?;
                    let split_cols = (((w as u32 * (100 - app.split_ratio as u32)) / 100) as u16)
                        .saturating_sub(1)
                        .max(1);
                    let split_rows = h.saturating_sub(3).max(1);
                    for tab in &mut app.tabs {
                        let _ = tab.pty.resize(split_rows, split_cols);
                    }
                }
                AppEvent::PtyOutput { tab_id, data } => {
                    app.on_pty_output(tab_id, &data);
                }
                AppEvent::PtyExit { tab_id } => {
                    app.on_pty_exit(tab_id);
                    if app.should_quit {
                        should_render = false;
                    }
                }
                AppEvent::AgentChunk(chunk) => app.on_agent_chunk(chunk),
                AppEvent::AgentDone => app.on_agent_done(),
                AppEvent::AgentUsage {
                    prompt_tokens,
                    completion_tokens,
                    exact_speed,
                } => {
                    app.on_agent_usage(prompt_tokens, completion_tokens, exact_speed);
                }
                AppEvent::McpServersUpdated => app.on_mcp_servers_updated(),
                AppEvent::AgentError(err) => app.on_agent_error(err),
                AppEvent::AgentToolRequest {
                    command,
                    approval_tx,
                } => {
                    app.on_agent_tool_request(command, approval_tx);
                }
                AppEvent::AgentToolStart(cmd) => app.on_agent_tool_start(cmd),
                AppEvent::AgentToolDone { command, output } => {
                    app.on_agent_tool_done(command, output);
                }
                AppEvent::AgentPtyToolExecute { command, result_tx } => {
                    app.on_agent_pty_tool_execute(command, result_tx, false);
                }
                AppEvent::AgentNewTurn => app.on_agent_new_turn(),
                AppEvent::ModelsLoaded {
                    provider_key,
                    models,
                } => {
                    app.on_models_loaded(provider_key, models);
                }
                AppEvent::ModelsLoadFailed {
                    provider_key,
                    error,
                } => {
                    app.on_models_load_failed(provider_key, error);
                }
                AppEvent::RemoteHostProbed { target, output } => {
                    app.on_remote_host_probed(target, output);
                }
                AppEvent::UpdatePricing => {
                    app.trigger_pricing_update();
                }
                AppEvent::PricingUpdated(res) => {
                    app.on_pricing_updated(res);
                }
                AppEvent::Tick => {
                    app.on_tick();
                    // Only re-render on Tick if a spinner animation is actively running or dragging
                    should_render = app.agent.is_generating
                        || app.active_pty_tool.is_some()
                        || app.is_dragging_split;
                }
            }

            if should_render && !app.should_quit {
                terminal.draw(|f| {
                    ui::draw(f, app);
                })?;
            }
        }
    }

    // Persist current session context and history before exiting
    app.save_current_session();

    Ok(())
}

fn setup_panic_hook() {
    let original_hook = panic::take_hook();
    panic::set_hook(Box::new(move |panic_info| {
        // Restore terminal so the error doesn't corrupt the terminal screen
        let _ = disable_raw_mode();
        let _ = execute!(
            io::stdout(),
            LeaveAlternateScreen,
            DisableBracketedPaste,
            DisableMouseCapture,
            SetCursorStyle::DefaultUserShape
        );

        // Write panic details to ~/.config/spiritty/crash.log
        if let Some(config_dir) = dirs::config_dir() {
            let spiritty_dir = config_dir.join("spiritty");
            let _ = std::fs::create_dir_all(&spiritty_dir);
            let crash_file = spiritty_dir.join("crash.log");
            let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
            let payload = format!(
                "=== Crash at {} ===\n{}\nBacktrace:\n{:?}\n\n",
                timestamp,
                panic_info,
                std::backtrace::Backtrace::capture()
            );
            if let Ok(mut f) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(crash_file)
            {
                use std::io::Write;
                let _ = writeln!(f, "{}", payload);
            }
        }

        original_hook(panic_info);
    }));
}

/// Spawns a thread that watches SIGTERM / SIGINT / SIGHUP, restores the terminal the
/// same way the panic hook does, and exits with the conventional `128 + signal` status.
/// This runs even when the main loop is wedged (a stalled blocking call is exactly the
/// situation where a user kills the process), so an externally killed Spiritty no longer
/// leaves the terminal in raw mode.
fn setup_termination_signal_hook() {
    use signal_hook::consts::{SIGHUP, SIGINT, SIGTERM};

    let mut signals = match signal_hook::iterator::Signals::new([SIGHUP, SIGINT, SIGTERM]) {
        Ok(signals) => signals,
        // Signal registration is best-effort: without it the previous behavior applies.
        Err(_) => return,
    };
    thread::spawn(move || {
        // A single signal is handled: after restoring the terminal the process exits
        // with the conventional `128 + signal` status.
        if let Some(sig) = signals.forever().next() {
            let _ = disable_raw_mode();
            let _ = execute!(
                io::stdout(),
                LeaveAlternateScreen,
                DisableBracketedPaste,
                DisableMouseCapture,
                SetCursorStyle::DefaultUserShape
            );
            let _ = execute!(io::stdout(), crossterm::event::PopKeyboardEnhancementFlags);
            let _ = execute!(io::stdout(), crossterm::cursor::Show);
            let code = match sig {
                SIGINT => 130,
                SIGHUP => 129,
                _ => 143,
            };
            std::process::exit(code);
        }
    });
}
