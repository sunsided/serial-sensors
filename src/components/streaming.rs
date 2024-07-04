use std::default::Default;
use std::sync::Arc;

use color_eyre::eyre::Result;
use crossterm::event::KeyEvent;
use ratatui::{prelude::*, widgets::*};
use serial_sensors_proto::versions::Version1DataFrame;
use tokio::sync::mpsc::UnboundedSender;

use crate::action::Action;
use crate::components::utils::frame_data_to_line;
use crate::data_buffer::SensorDataBuffer;

use super::{Component, Frame};

pub struct StreamingLog {
    action_tx: Option<UnboundedSender<Action>>,
    receiver: Arc<SensorDataBuffer>,
    recent: Vec<Version1DataFrame>,
}

impl StreamingLog {
    pub fn new(receiver: Arc<SensorDataBuffer>) -> Self {
        let capacity = receiver.capacity().min(60);
        Self {
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
            .constraints([Constraint::Min(10), Constraint::Percentage(100)].as_ref())
            .split(rect);
        let rect = rects[1];

        // Fetch the actual height of the window.
        let height = rects[1].height;

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

                frame_data_to_line(frame, &mut line);

                Line::from(line)
            })
            .collect();

        f.render_widget(
            Paragraph::new(log_rows)
                .left_aligned()
                .block(
                    Block::default()
                        .title("Streaming Log")
                        .title_alignment(Alignment::Left)
                        .borders(Borders::ALL)
                        .border_type(BorderType::Rounded),
                )
                .style(Style::default().fg(Color::Gray)),
            rect,
        );

        Ok(())
    }
}
