use std::default::Default;
use std::sync::Arc;

use color_eyre::eyre::Result;
use crossterm::event::KeyEvent;
use ratatui::{prelude::*, widgets::*};
use serial_sensors_proto::versions::Version1DataFrame;
use serial_sensors_proto::SensorData;
use tokio::sync::mpsc::UnboundedSender;

use crate::action::Action;
use crate::data_buffer::SensorDataBuffer;

use super::{Component, Frame};

pub struct StreamingLog {
    capacity: usize,
    action_tx: Option<UnboundedSender<Action>>,
    receiver: Arc<SensorDataBuffer>,
    recent: Vec<Version1DataFrame>,
}

impl StreamingLog {
    pub fn new(receiver: Arc<SensorDataBuffer>) -> Self {
        let capacity = receiver.capacity().min(60);
        Self {
            capacity,
            action_tx: None,
            receiver,
            recent: Vec::with_capacity(capacity),
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

        // Fetch the actual height of the window.
        let height = rects[0].height;

        // Obtain the most recent data.
        self.recent.clear();
        let capacity = height as usize;
        let len = self.receiver.clone_latest(capacity, &mut self.recent);

        let log_rows: Vec<Line> = self.recent[..len]
            .iter()
            .rev()
            .map(|frame| {
                let mut line = vec![
                    Span::styled(frame.global_sequence.to_string(), Style::default().dim()),
                    ", ".into(),
                    Span::styled(frame.sensor_tag.to_string(), Style::default().yellow()),
                    ":".into(),
                    Span::styled(frame.sensor_sequence.to_string(), Style::default().dim()),
                    " ".into(),
                    Span::styled(
                        format!("{:02X}", frame.value.sensor_type_id()),
                        Style::default().dim(),
                    ),
                    ":".into(),
                    Span::styled(
                        format!("{:02X}", frame.value.value_type() as u8),
                        Style::default().dim(),
                    ),
                    " ".into(),
                ];

                if let SensorData::AccelerometerI16(vec) = frame.value {
                    line.extend(vec![
                        Span::styled("acc", Style::default().cyan()),
                        " = (".into(),
                        Span::styled(
                            (vec.x as f32 / 16384.0).to_string(),
                            Style::default().white(),
                        ),
                        ", ".into(),
                        Span::styled(
                            (vec.y as f32 / 16384.0).to_string(),
                            Style::default().white(),
                        ),
                        ", ".into(),
                        Span::styled(
                            (vec.z as f32 / 16384.0).to_string(),
                            Style::default().white(),
                        ),
                        ")".into(),
                    ]);
                }

                Line::from(line)
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
                .style(Style::default().fg(Color::Gray)),
            rects[0],
        );

        Ok(())
    }
}
