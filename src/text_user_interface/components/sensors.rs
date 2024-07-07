use std::default::Default;
use std::sync::Arc;

use color_eyre::eyre::Result;
use ratatui::{prelude::*, widgets::*};
use tokio::sync::mpsc::UnboundedSender;

use crate::text_user_interface::action::Action;
use crate::text_user_interface::components::utils::frame_data_to_line;
use crate::text_user_interface::data_buffer::SensorDataBuffer;

use super::{Component, Frame};

pub struct Sensors {
    action_tx: Option<UnboundedSender<Action>>,
    receiver: Arc<SensorDataBuffer>,
}

impl Sensors {
    pub fn new(receiver: Arc<SensorDataBuffer>) -> Self {
        Self {
            action_tx: None,
            receiver,
        }
    }
}

impl Component for Sensors {
    fn register_action_handler(&mut self, tx: UnboundedSender<Action>) -> Result<()> {
        self.action_tx = Some(tx);
        Ok(())
    }

    fn update(&mut self, _action: Action) -> Result<Option<Action>> {
        Ok(None)
    }

    fn draw(&mut self, f: &mut Frame<'_>, rect: Rect) -> Result<()> {
        let rects = Layout::default()
            .constraints([Constraint::Length(10)].as_ref())
            .split(rect);

        // Get all sensor types.
        let sensors = self.receiver.get_sensors();

        let rows: Vec<Line> = sensors
            .into_iter()
            .map(|id| (id.clone(), self.receiver.get_latest_by_sensor(&id)))
            .filter(|(_, frame)| frame.is_some())
            .map(|(id, frame)| (id, frame.expect("value exists")))
            .enumerate()
            .map(|(no, (id, frame))| {
                // TODO: IF time is supported. :)
                let time = frame.system_secs as f32 + frame.system_millis as f32 / 1000.0;

                let avg_duration = self
                    .receiver
                    .get_average_duration_by_sensor(&id)
                    .unwrap_or_default();
                let fps = avg_duration.as_secs_f32().recip();

                let skipped = self.receiver.get_skipped_by_sensor(&id);
                let skipped = if skipped > 0 {
                    Span::styled(format!("/{skipped}"), Style::default().red())
                } else {
                    "".into()
                };

                let name = self.receiver.get_sensor_name(&id);
                let name = if !name.is_empty() {
                    Span::styled(name, Style::default().white().underlined())
                } else {
                    "unknown".into()
                };

                let mut lines = vec![
                    Span::styled(format!("{no}"), Style::default()),
                    ": ".into(),
                    Span::styled(format!("t={:3.3}", time), Style::default().dim()),
                    " ".into(),
                    name,
                    Span::styled(format!(" {}", id.tag()), Style::default().yellow()),
                    ":".into(),
                    Span::styled(frame.sensor_sequence.to_string(), Style::default().dim()),
                    skipped,
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
                    " (".into(),
                    Span::styled(format!("{:2.2}", fps), Style::default()),
                    " Hz) ".into(),
                ];

                frame_data_to_line(&id, &self.receiver, &frame, &mut lines);
                lines
            })
            .map(|lines| lines.into())
            .collect();

        f.render_widget(
            Paragraph::new(rows)
                .left_aligned()
                .block(
                    Block::default()
                        .title("Sensors")
                        .title_alignment(Alignment::Left)
                        .borders(Borders::ALL)
                        .border_type(BorderType::Rounded),
                )
                .style(Style::default().fg(Color::Gray)),
            rects[0],
        );

        Ok(())
    }
}
