use crossterm::event::{self, Event as CrosstermEvent, KeyCode, KeyEvent};
use std::time::Duration;
use tokio::sync::mpsc;

#[derive(Debug, Clone)]
pub enum Event {
    Key(KeyEvent),
    Tick,
}

pub struct EventHandler {
    rx: mpsc::UnboundedReceiver<Event>,
    shutdown_tx: mpsc::UnboundedSender<()>,
}

impl Default for EventHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl EventHandler {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        let (shutdown_tx, mut shutdown_rx) = mpsc::unbounded_channel();

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = shutdown_rx.recv() => {
                        break;
                    }
                    _ = tokio::time::sleep(Duration::from_millis(16)) => {
                        if event::poll(Duration::from_millis(0)).unwrap_or(false) {
                            if let Ok(CrosstermEvent::Key(key)) = event::read() {
                                if tx.send(Event::Key(key)).is_err() {
                                    break;
                                }
                            }
                        } else if tx.send(Event::Tick).is_err() {
                            break;
                        }
                    }
                }
            }
        });

        Self { rx, shutdown_tx }
    }

    pub async fn next(&mut self) -> Option<Event> {
        self.rx.recv().await
    }

    pub fn stop(&self) {
        let _ = self.shutdown_tx.send(());
    }
}

pub fn should_quit(key: &KeyEvent) -> bool {
    matches!(key.code, KeyCode::Char('q') | KeyCode::Char('Q'))
}
