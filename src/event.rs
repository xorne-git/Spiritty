use crossterm::event::{Event as CrosstermEvent, EventStream, KeyEvent, KeyEventKind, MouseEvent};
use futures::StreamExt;
use std::time::Duration;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};

#[derive(Debug)]
#[allow(dead_code)]
pub enum AppEvent {
    Key(KeyEvent),
    Mouse(MouseEvent),
    Paste(String),
    Resize(u16, u16),
    PtyOutput(Vec<u8>),
    PtyExit,
    Tick,
    AgentChunk(String),
    AgentDone,
    AgentError(String),
    AgentToolRequest {
        command: String,
        approval_tx: tokio::sync::oneshot::Sender<bool>,
    },
    AgentToolStart(String),
    AgentToolDone { command: String, output: String },
    AgentPtyToolExecute {
        command: String,
        result_tx: tokio::sync::oneshot::Sender<String>,
    },
    AgentNewTurn,
    ModelsLoaded {
        provider_key: String,
        models: Vec<String>,
    },
}

pub struct EventHandler {
    sender: UnboundedSender<AppEvent>,
    receiver: UnboundedReceiver<AppEvent>,
}

impl EventHandler {
    pub fn new(tick_rate: Duration) -> Self {
        let (sender, receiver) = mpsc::unbounded_channel();
        let event_tx = sender.clone();

        // Task for capturing crossterm terminal events
        tokio::spawn(async move {
            let mut reader = EventStream::new();
            loop {
                let event = reader.next().await;
                match event {
                    Some(Ok(CrosstermEvent::Key(key))) => {
                        // Ignore standalone modifier keys (Shift, Ctrl, Alt) so they don't produce ghost events
                        if matches!(key.code, crossterm::event::KeyCode::Modifier(_)) {
                            continue;
                        }
                        // Only handle Press events to avoid double trigger on Windows/Linux
                        if key.kind == KeyEventKind::Press && event_tx.send(AppEvent::Key(key)).is_err() {
                            break;
                        }
                    }
                    Some(Ok(CrosstermEvent::Paste(text))) => {
                        if event_tx.send(AppEvent::Paste(text)).is_err() {
                            break;
                        }
                    }
                    Some(Ok(CrosstermEvent::Mouse(mouse))) => {
                        // Ignore unclicked hover move events to eliminate event flooding and guarantee instant UI responsiveness
                        if mouse.kind != crossterm::event::MouseEventKind::Moved
                            && event_tx.send(AppEvent::Mouse(mouse)).is_err()
                        {
                            break;
                        }
                    }
                    Some(Ok(CrosstermEvent::Resize(w, h))) => {
                        if event_tx.send(AppEvent::Resize(w, h)).is_err() {
                            break;
                        }
                    }
                    Some(Err(_)) => {
                        break;
                    }
                    _ => {}
                }
            }
        });

        // Task for tick rate (e.g. 60 FPS / 16ms or 30 FPS)
        let tick_tx = sender.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tick_rate);
            loop {
                interval.tick().await;
                if tick_tx.send(AppEvent::Tick).is_err() {
                    break;
                }
            }
        });

        Self { sender, receiver }
    }

    pub fn sender(&self) -> UnboundedSender<AppEvent> {
        self.sender.clone()
    }

    pub async fn next(&mut self) -> Option<AppEvent> {
        self.receiver.recv().await
    }
}
