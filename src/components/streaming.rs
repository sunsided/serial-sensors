use std::{collections::HashMap, time::Duration};

use color_eyre::eyre::Result;
use crossterm::event::{KeyCode, KeyEvent};
use log::error;
use ratatui::{prelude::*, widgets::*};
use serial_sensors_proto::types::AccelerometerI16;
use serial_sensors_proto::versions::Version1DataFrame;
use tokio::sync::mpsc::UnboundedSender;
use tracing::trace;
use tui_input::{backend::crossterm::EventHandler, Input};

use crate::{action::Action, config::key_event_to_string};

use super::{Component, Frame};

#[derive(Default, Copy, Clone, PartialEq, Eq)]
pub enum Mode {
    #[default]
    Normal,
    Insert,
    Processing,
}

#[derive(Default)]
pub struct StreamingLog {
    pub mode: Mode,
    pub input: Input,
    pub action_tx: Option<UnboundedSender<Action>>,
    pub keymap: HashMap<KeyEvent, Action>,
    pub data: Vec<Version1DataFrame>,
    pub last_events: Vec<KeyEvent>,
}

impl StreamingLog {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn keymap(mut self, keymap: HashMap<KeyEvent, Action>) -> Self {
        self.keymap = keymap;
        self
    }

    pub fn add(&mut self, s: Version1DataFrame) {
        self.data.push(s)
    }
}

impl Component for StreamingLog {
    fn register_action_handler(&mut self, tx: UnboundedSender<Action>) -> Result<()> {
        self.action_tx = Some(tx);
        Ok(())
    }

    fn handle_key_events(&mut self, key: KeyEvent) -> Result<Option<Action>> {
        self.last_events.push(key);
        let action = match self.mode {
            Mode::Normal | Mode::Processing => return Ok(None),
            Mode::Insert => match key.code {
                KeyCode::Esc => Action::EnterNormal,
                KeyCode::Enter => {
                    if let Some(sender) = &self.action_tx {
                        if let Err(e) =
                            sender.send(Action::CompleteInput(self.input.value().to_string()))
                        {
                            error!("Failed to send action: {:?}", e);
                        }
                    }
                    Action::EnterNormal
                }
                _ => {
                    self.input.handle_event(&crossterm::event::Event::Key(key));
                    Action::Update
                }
            },
        };
        Ok(Some(action))
    }

    fn update(&mut self, action: Action) -> Result<Option<Action>> {
        match action {
            Action::SensorRow(s) => self.add(s),
            _ => (),
        }
        Ok(None)
    }

    fn draw(&mut self, f: &mut Frame<'_>, rect: Rect) -> Result<()> {
        let rects = Layout::default()
            .constraints([Constraint::Percentage(100), Constraint::Min(5)].as_ref())
            .split(rect);

        let text: Vec<Line> = self
            .data
            .clone()
            .iter()
            .map(|frame| {
                let header = format!(
                    "{}, {}:{} {:02X}:{:02X} ",
                    frame.global_sequence,
                    frame.sensor_tag,
                    frame.sensor_sequence,
                    frame.value.sensor_type_id(),
                    frame.value.value_type() as u8,
                );

                let vec: AccelerometerI16 = frame.value.clone().try_into().unwrap();

                let str = format!(
                    "acc = ({:.04}, {:.04}, {:.04})",
                    vec.x as f32 / 16384.0,
                    vec.y as f32 / 16384.0,
                    vec.z as f32 / 16384.0
                );

                let l = format!("{header} {str}");

                Line::from(l)
            })
            .collect();

        let width = rects[1].width.max(3) - 3; // keep 2 for borders and 1 for cursor
        let scroll = self.input.visual_scroll(width as usize);

        f.render_widget(
            Paragraph::new(text)
                .block(
                    Block::default()
                        .title("Streaming Log")
                        .title_alignment(Alignment::Center)
                        .borders(Borders::ALL)
                        .border_type(BorderType::Rounded),
                )
                .style(Style::default().fg(Color::Gray))
                .alignment(Alignment::Left)
                .scroll((0, scroll as u16)),
            rects[0],
        );

        Ok(())
    }
}
