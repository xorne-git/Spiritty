use anyhow::Result;
use crossterm::{
    cursor::{EnableBlinking, SetCursorStyle},
    event::{
        DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture,
    },
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
    panic,
    time::Duration,
};

#[tokio::main]
async fn main() -> Result<()> {
    // Setup panic hook to always restore terminal state on panic
    setup_panic_hook();

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

    let supports_enhancement = crossterm::terminal::supports_keyboard_enhancement().unwrap_or(false);
    if supports_enhancement {
        let _ = execute!(
            stdout,
            crossterm::event::PushKeyboardEnhancementFlags(
                crossterm::event::KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
            )
        );
    }

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Calculate initial split dimensions for the PTY (full screen height)
    let term_size = terminal.size()?;
    let split_ratio = 50u16;
    let initial_rows = term_size.height.saturating_sub(3).max(1);
    let initial_cols = (((term_size.width as u32 * (100 - split_ratio as u32)) / 100) as u16)
        .saturating_sub(1)
        .max(1);

    // Create event loop and application state
    let tick_rate = Duration::from_millis(90); // ~11 FPS for smooth, balanced spinner cadence
    let mut event_handler = EventHandler::new(tick_rate);
    let mut app = App::new(event_handler.sender(), initial_rows, initial_cols)?;

    // Main event loop
    let res = run_loop(&mut terminal, &mut app, &mut event_handler).await;

    // Restore terminal state cleanly
    let _ = execute!(terminal.backend_mut(), SetCursorStyle::DefaultUserShape);
    if supports_enhancement {
        let _ = execute!(terminal.backend_mut(), crossterm::event::PopKeyboardEnhancementFlags);
    }
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
                    let _ = app.pty.resize(split_rows, split_cols);
                }
                AppEvent::PtyOutput(bytes) => {
                    app.on_pty_output(&bytes);
                }
                AppEvent::PtyExit => {
                    app.should_quit = true;
                    should_render = false;
                }
                AppEvent::AgentChunk(chunk) => app.on_agent_chunk(chunk),
                AppEvent::AgentDone => app.on_agent_done(),
                AppEvent::AgentError(err) => app.on_agent_error(err),
                AppEvent::AgentToolRequest { command, approval_tx } => {
                    app.on_agent_tool_request(command, approval_tx);
                }
                AppEvent::AgentToolStart(cmd) => app.on_agent_tool_start(cmd),
                AppEvent::AgentToolDone { command, output } => {
                    app.on_agent_tool_done(command, output);
                }
                AppEvent::AgentPtyToolExecute { command, result_tx } => {
                    app.on_agent_pty_tool_execute(command, result_tx);
                }
                AppEvent::AgentNewTurn => app.on_agent_new_turn(),
                AppEvent::ModelsLoaded { provider_key, models } => {
                    app.on_models_loaded(provider_key, models);
                }
                AppEvent::Tick => {
                    app.on_tick();
                    // Only re-render on Tick if a spinner animation is actively running or dragging
                    should_render = app.agent.is_generating || app.active_pty_tool.is_some() || app.is_dragging_split;
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
        original_hook(panic_info);
    }));
}
