use crossterm::event::{KeyCode, KeyEventKind};
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};

use crate::error::InstallerResult;

use super::utils::centered_rect;
use super::{Event, Tui};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CurrentScreen {
    #[default]
    Main,
    Exiting,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppAction {
    Exit(bool),
    SetScreen(CurrentScreen),
}

pub trait Screen: std::fmt::Debug + Clone + Default + PartialEq + Eq {
    fn ui(&self, f: &mut Frame);
    fn handle_event(&self, evt: Event) -> Option<AppAction>;
    fn update(&mut self, action: AppAction) -> Option<AppAction>;
}

#[derive(Debug, PartialEq, Eq, Default)]
pub struct App {
    pub should_quit: bool,
    pub current_screen: CurrentScreen,
    pub main_screen: MainScreen,
    pub exiting_screen: ExitingScreen,
}

impl App {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn run(&mut self) -> InstallerResult<()> {
        let mut tui = Tui::new()?
            .tick_rate(4.0) // 4 ticks per second
            .frame_rate(30.0) // 30 frames per second
            .mouse(true) // enable mouse events
            .paste(true); // enable paste events

        tui.enter()?; // Starts event handler, enters raw mode, enters alternate screen

        loop {
            tui.draw(|f| {
                // Deref allows calling `tui.terminal.draw`
                self.ui(f);
            })?;

            if let Some(evt) = tui.next().await {
                // `tui.next().await` blocks till next event
                let mut maybe_action = self.handle_event(evt);
                while let Some(action) = maybe_action {
                    maybe_action = self.update(action);
                }
            };

            if self.should_quit {
                break;
            }
        }

        tui.exit()?; // stops event handler, exits raw mode, exits alternate screen

        Ok(())
    }

    fn ui(&mut self, f: &mut Frame) {
        match self.current_screen {
            CurrentScreen::Main => self.main_screen.ui(f),
            CurrentScreen::Exiting => self.exiting_screen.ui(f),
        }
    }

    fn handle_event(&self, evt: Event) -> Option<AppAction> {
        match evt {
            Event::Key(key) if key.kind == KeyEventKind::Release => None,
            _ => match self.current_screen {
                CurrentScreen::Main => self.main_screen.handle_event(evt),
                CurrentScreen::Exiting => self.exiting_screen.handle_event(evt),
            },
        }
    }

    fn update(&mut self, action: AppAction) -> Option<AppAction> {
        let next_action = match self.current_screen {
            CurrentScreen::Main => self.main_screen.update(action),
            CurrentScreen::Exiting => self.exiting_screen.update(action),
        };
        if let Some(na) = next_action {
            return Some(na);
        }
        match action {
            AppAction::Exit(_) => {
                self.should_quit = true;
                None
            }
            AppAction::SetScreen(screen) => {
                self.current_screen = screen;
                None
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MainScreen {
    pub current_screen: CurrentScreen,
}

impl Screen for MainScreen {
    fn ui(&self, f: &mut Frame) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(1),
                Constraint::Length(3),
            ])
            .split(f.area());

        let title_block = Block::default()
            .borders(Borders::ALL)
            .style(Style::default());

        let title = Paragraph::new(Text::styled(
            "Create New Json",
            Style::default().fg(Color::Green),
        ))
        .block(title_block);

        f.render_widget(title, chunks[0]);

        let current_navigation_text = match self.current_screen {
            CurrentScreen::Main => Span::styled("Normal Mode", Style::default().fg(Color::Green)),
            CurrentScreen::Exiting => Span::styled("Exiting", Style::default().fg(Color::LightRed)),
        }
        .to_owned();

        let mode_footer = Paragraph::new(Line::from(current_navigation_text))
            .block(Block::default().borders(Borders::ALL));

        let current_keys_hint = {
            match self.current_screen {
                CurrentScreen::Main => Span::styled("(q) to quit", Style::default().fg(Color::Red)),
                CurrentScreen::Exiting => {
                    Span::styled("(q) to quit", Style::default().fg(Color::Red))
                }
            }
        };

        let key_notes_footer = Paragraph::new(Line::from(current_keys_hint))
            .block(Block::default().borders(Borders::ALL));

        let footer_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(chunks[2]);

        f.render_widget(mode_footer, footer_chunks[0]);
        f.render_widget(key_notes_footer, footer_chunks[1]);
    }

    fn handle_event(&self, evt: Event) -> Option<AppAction> {
        match evt {
            Event::Key(key) if key.kind != KeyEventKind::Release => match key.code {
                KeyCode::Char('q') => Some(AppAction::SetScreen(CurrentScreen::Exiting)),
                _ => None,
            },
            _ => None,
        }
    }

    fn update(&mut self, action: AppAction) -> Option<AppAction> {
        match action {
            AppAction::SetScreen(screen) => {
                self.current_screen = screen;
                None
            }
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ExitingScreen;

impl Screen for ExitingScreen {
    fn ui(&self, f: &mut Frame) {
        f.render_widget(Clear, f.area());
        let popup_block = Block::default()
            .title("Y/N")
            .borders(Borders::NONE)
            .style(Style::default().bg(Color::DarkGray));

        let exit_text = Text::styled(
            "Would you like to output the settings? (y/n)",
            Style::default().fg(Color::Red),
        );
        // the `trim: false` will stop the text from being cut off when over the edge of the block
        let exit_paragraph = Paragraph::new(exit_text)
            .block(popup_block)
            .wrap(Wrap { trim: false });

        let area = centered_rect(60, 25, f.area());
        f.render_widget(exit_paragraph, area);
    }

    fn handle_event(&self, evt: Event) -> Option<AppAction> {
        match evt {
            Event::Key(key) => match key.code {
                KeyCode::Char('y') => Some(AppAction::Exit(true)),
                KeyCode::Char('n') | KeyCode::Char('q') => Some(AppAction::Exit(false)),
                _ => None,
            },
            _ => None,
        }
    }

    fn update(&mut self, action: AppAction) -> Option<AppAction> {
        match action {
            AppAction::Exit(should_output) => {
                if should_output {
                    println!("Outputting settings...");
                }
                None
            }
            _ => None,
        }
    }
}
