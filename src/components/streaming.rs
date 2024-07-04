use std::default::Default;

use color_eyre::eyre::Result;
use crossterm::event::KeyEvent;
use ratatui::{prelude::*, widgets::*};
use serial_sensors_proto::SensorData;
use serial_sensors_proto::versions::Version1DataFrame;
use tokio::sync::mpsc::UnboundedSender;

use crate::action::Action;

use super::{Component, Frame};

pub struct StreamingLog {
    pub action_tx: Option<UnboundedSender<Action>>,
    pub data: Vec<Version1DataFrame>,
}

impl StreamingLog {
    pub fn new() -> Self {
        Self {
            action_tx: None,
            data: Vec::new(),
        }
    }

    fn add(&mut self, s: Version1DataFrame) {
        self.data.push(s)
    }
}

impl Component for StreamingLog {
    fn register_action_handler(&mut self, tx: UnboundedSender<Action>) -> Result<()> {
        self.action_tx = Some(tx);
        Ok(())
    }

    fn handle_key_events(&mut self, _key: KeyEvent) -> Result<Option<Action>> {
        // TODO: Add action to clear the buffer?
        Ok(None)
    }

    fn update(&mut self, action: Action) -> Result<Option<Action>> {
        if let Action::SensorRow(s) = action {
            self.add(s)
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

                let payload = if let SensorData::AccelerometerI16(vec) = frame.value {
                    format!(
                        "acc = ({:.04}, {:.04}, {:.04})",
                        vec.x as f32 / 16384.0,
                        vec.y as f32 / 16384.0,
                        vec.z as f32 / 16384.0
                    )
                } else {
                    String::new()
                };

                let l = format!("{header} {payload}");

                Line::from(l)
            })
            .collect();

        f.render_widget(
            Paragraph::new(text)
                .left_aligned()
                .block(
                    Block::default()
                        .title("Streaming Log")
                        .title_alignment(Alignment::Center)
                        .borders(Borders::ALL)
                        .border_type(BorderType::Rounded),
                )
                .style(Style::default().fg(Color::Gray)),
            rects[0],
        );

        Ok(())
    }
}
