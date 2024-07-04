use std::default::Default;
use std::sync::Arc;

use color_eyre::eyre::Result;
use crossterm::event::KeyEvent;
use ratatui::{prelude::*, widgets::*};
use serial_sensors_proto::SensorData;
use tokio::sync::mpsc::UnboundedSender;

use crate::action::Action;
use crate::data_buffer::SensorDataBuffer;

use super::{Component, Frame};

pub struct StreamingLog {
    pub action_tx: Option<UnboundedSender<Action>>,
    pub receiver: Arc<SensorDataBuffer>,
}

impl StreamingLog {
    pub fn new(receiver: Arc<SensorDataBuffer>) -> Self {
        Self {
            action_tx: None,
            receiver,
        }
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

    fn update(&mut self, _action: Action) -> Result<Option<Action>> {
        Ok(None)
    }

    fn draw(&mut self, f: &mut Frame<'_>, rect: Rect) -> Result<()> {
        let rects = Layout::default()
            .constraints([Constraint::Percentage(100), Constraint::Min(5)].as_ref())
            .split(rect);

        // Let's not talk about this.
        const CAPACITY: usize = 20;
        let mut data = Vec::with_capacity(CAPACITY);
        let len = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current()
                .block_on(self.receiver.clone_latest(CAPACITY, &mut data))
        });

        let log_rows: Vec<Line> = data[..len]
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
            Paragraph::new(log_rows)
                .left_aligned()
                .block(
                    Block::default()
                        .title("Streaming Log")
                        .title_alignment(Alignment::Center)
                        .borders(Borders::ALL)
                        .border_type(BorderType::Rounded),
                )
                .scroll((5, 0))
                .style(Style::default().fg(Color::Gray)),
            rects[0],
        );

        Ok(())
    }
}
