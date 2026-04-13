pub mod app;
pub mod utils;

use std::io;
use std::ops::{Deref, DerefMut};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::sleep;
use std::time::Duration;

use crossterm::event::EventStream;
use crossterm::execute;
use crossterm::terminal::{disable_raw_mode, enable_raw_mode, is_raw_mode_enabled};
use futures::{FutureExt, StreamExt};
use ratatui::crossterm::cursor;
use ratatui::crossterm::event::{
    DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture,
    Event as CrosstermEvent, KeyEvent, KeyEventKind, MouseEvent,
};
use ratatui::crossterm::terminal::{EnterAlternateScreen, LeaveAlternateScreen};
use ratatui::prelude::CrosstermBackend;
use ratatui::{DefaultTerminal, Terminal};
use serde::{Deserialize, Serialize};
#[cfg(not(windows))]
use signal_hook::consts::SIGTSTP;
#[cfg(not(windows))]
use signal_hook::low_level::raise;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};
use tokio::task::JoinHandle;
use tokio::time;
use tokio_util::sync::CancellationToken;
use tracing::error;

use crate::error::{InstallerError, InstallerResult};

static PASTE: AtomicBool = AtomicBool::new(false);
static MOUSE: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Event {
    Init,
    Quit,
    Error,
    Closed,
    Tick,
    Render,
    FocusGained,
    FocusLost,
    Paste(String),
    Key(KeyEvent),
    Mouse(MouseEvent),
    Resize(u16, u16),
}

pub struct Tui {
    pub terminal: DefaultTerminal,
    pub task: JoinHandle<()>,
    pub cancellation_token: CancellationToken,
    pub event_rx: UnboundedReceiver<Event>,
    pub event_tx: UnboundedSender<Event>,
    pub frame_rate: f64,
    pub tick_rate: f64,
    pub mouse: bool,
    pub paste: bool,
}

impl Tui {
    pub fn new() -> InstallerResult<Self> {
        let tick_rate = 4.0;
        let frame_rate = 60.0;
        let terminal =
            Terminal::new(CrosstermBackend::new(io::stdout())).map_err(InstallerError::Create)?;
        let (event_tx, event_rx) = unbounded_channel();
        let cancellation_token = CancellationToken::new();
        let task = tokio::spawn(async {});
        let mouse = false;
        let paste = false;
        Ok(Self {
            terminal,
            task,
            cancellation_token,
            event_rx,
            event_tx,
            frame_rate,
            tick_rate,
            mouse,
            paste,
        })
    }

    pub fn tick_rate(mut self, tick_rate: f64) -> Self {
        self.tick_rate = tick_rate;
        self
    }

    pub fn frame_rate(mut self, frame_rate: f64) -> Self {
        self.frame_rate = frame_rate;
        self
    }

    pub fn mouse(mut self, mouse: bool) -> Self {
        self.mouse = mouse;
        self
    }

    pub fn paste(mut self, paste: bool) -> Self {
        self.paste = paste;
        self
    }

    pub fn start(&mut self) {
        let tick_delay = Duration::from_secs_f64(1.0 / self.tick_rate);
        let render_delay = Duration::from_secs_f64(1.0 / self.frame_rate);
        self.cancel();
        self.cancellation_token = CancellationToken::new();
        let cancellation_token = self.cancellation_token.clone();
        let event_tx = self.event_tx.clone();
        self.task = tokio::spawn(async move {
            let mut reader = EventStream::new();
            let mut tick_interval = time::interval(tick_delay);
            let mut render_interval = time::interval(render_delay);
            event_tx.send(Event::Init).unwrap();
            loop {
                let tick_delay = tick_interval.tick();
                let render_delay = render_interval.tick();
                let crossterm_event = reader.next().fuse();
                tokio::select! {
                  _ = cancellation_token.cancelled() => {
                    break;
                  }
                  maybe_event = crossterm_event => {
                    match maybe_event {
                      Some(Ok(evt)) => {
                        match evt {
                          CrosstermEvent::Key(key) => {
                            if key.kind == KeyEventKind::Press {
                              event_tx.send(Event::Key(key)).unwrap();
                            }
                          },
                          CrosstermEvent::Mouse(mouse) => {
                            event_tx.send(Event::Mouse(mouse)).unwrap();
                          },
                          CrosstermEvent::Resize(x, y) => {
                            event_tx.send(Event::Resize(x, y)).unwrap();
                          },
                          CrosstermEvent::FocusLost => {
                            event_tx.send(Event::FocusLost).unwrap();
                          },
                          CrosstermEvent::FocusGained => {
                            event_tx.send(Event::FocusGained).unwrap();
                          },
                          CrosstermEvent::Paste(s) => {
                            event_tx.send(Event::Paste(s)).unwrap();
                          },
                        }
                      }
                      Some(Err(_)) => {
                        event_tx.send(Event::Error).unwrap();
                      }
                      None => {},
                    }
                  },
                  _ = tick_delay => {
                      event_tx.send(Event::Tick).unwrap();
                  },
                  _ = render_delay => {
                      event_tx.send(Event::Render).unwrap();
                  },
                }
            }
        });
    }

    pub fn stop(&self) -> InstallerResult<()> {
        self.cancel();
        let mut counter = 0;
        while !self.task.is_finished() {
            sleep(Duration::from_millis(1));
            counter += 1;
            if counter > 50 {
                self.task.abort();
            }
            if counter > 100 {
                error!("Failed to abort task in 100 milliseconds for unknown reason");
                break;
            }
        }
        Ok(())
    }

    pub fn enter(&mut self) -> InstallerResult<()> {
        enable_raw_mode().map_err(InstallerError::InitRawMode)?;
        execute!(io::stdout(), EnterAlternateScreen, cursor::Hide)
            .map_err(InstallerError::InitExec)?;
        if self.mouse {
            MOUSE.store(true, Ordering::Relaxed);
            execute!(io::stdout(), EnableMouseCapture).map_err(InstallerError::InitMouseCapture)?;
        }
        if self.paste {
            PASTE.store(true, Ordering::Relaxed);
            execute!(io::stdout(), EnableBracketedPaste).map_err(InstallerError::InitPaste)?;
        }
        self.start();
        Ok(())
    }

    pub fn exit(&mut self) -> InstallerResult<()> {
        self.stop()?;
        if is_raw_mode_enabled().map_err(InstallerError::DeinitRawMode)? {
            self.flush().unwrap();
            if self.paste {
                execute!(io::stdout(), DisableBracketedPaste)
                    .map_err(InstallerError::DeinitPaste)?;
                PASTE.store(false, Ordering::Relaxed);
            }
            if self.mouse {
                execute!(io::stdout(), DisableMouseCapture)
                    .map_err(InstallerError::DeinitMouseCapture)?;
                MOUSE.store(false, Ordering::Relaxed);
            }
            execute!(io::stdout(), LeaveAlternateScreen, cursor::Show)
                .map_err(InstallerError::DeinitExec)?;
            disable_raw_mode().map_err(InstallerError::DeinitRawMode)?;
        }
        Ok(())
    }

    pub fn cancel(&self) {
        self.cancellation_token.cancel();
    }

    pub fn suspend(&mut self) -> InstallerResult<()> {
        self.exit()?;
        #[cfg(not(windows))]
        raise(SIGTSTP).map_err(InstallerError::Suspend)?;
        Ok(())
    }

    pub fn resume(&mut self) -> InstallerResult<()> {
        self.enter()?;
        Ok(())
    }

    pub async fn next(&mut self) -> Option<Event> {
        self.event_rx.recv().await
    }
}

impl Deref for Tui {
    type Target = DefaultTerminal;

    fn deref(&self) -> &Self::Target {
        &self.terminal
    }
}

impl DerefMut for Tui {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.terminal
    }
}

impl Drop for Tui {
    fn drop(&mut self) {
        self.exit().unwrap();
    }
}
