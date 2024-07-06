use std::default::Default;
use std::sync::Arc;

use color_eyre::eyre::Result;
use ratatui::{prelude::*, widgets::*};
use serial_sensors_proto::versions::Version1DataFrame;
use tokio::sync::mpsc::UnboundedSender;

use crate::action::Action;
use crate::components::utils::frame_data_to_line_raw;
use crate::data_buffer::SensorDataBuffer;

use super::{Component, Frame};

pub struct StreamingLog {
    action_tx: Option<UnboundedSender<Action>>,
    receiver: Arc<SensorDataBuffer>,
    recent: Vec<Version1DataFrame>,
    should_pause: bool,
}

impl StreamingLog {
    pub fn new(receiver: Arc<SensorDataBuffer>) -> Self {
        let capacity = receiver.capacity().min(60);
        Self {
            action_tx: None,
            receiver,
            recent: Vec::with_capacity(capacity),
            should_pause: false,
        }
    }
}

impl Component for StreamingLog {
    fn register_action_handler(&mut self, tx: UnboundedSender<Action>) -> Result<()> {
        self.action_tx = Some(tx);
        Ok(())
    }

    fn update(&mut self, action: Action) -> Result<Option<Action>> {
        match action {
            Action::Pause => self.should_pause = true,
            Action::Unpause => self.should_pause = false,
            _ => {}
        }
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
        let capacity = height as usize;
        let len = if !self.should_pause {
            self.recent.clear();
            self.receiver.clone_latest(capacity, &mut self.recent)
        } else {
            self.recent.len()
        };

        let log_rows: Vec<Line> = self.recent[..len]
            .iter()
            .rev()
            .map(|frame| {
                // TODO: IF time is supported. :)
                let time = frame.system_secs as f32 + frame.system_millis as f32 / 1000.0;

                let mut line = vec![
                    Span::styled(format!("t={:3.3}", time), Style::default().dim()),
                    " ".into(),
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

                frame_data_to_line_raw(frame, &mut line);

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
