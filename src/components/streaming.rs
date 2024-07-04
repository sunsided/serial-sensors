use std::default::Default;
use std::fmt::Display;
use std::ops::Neg;
use std::sync::Arc;

use color_eyre::eyre::Result;
use crossterm::event::KeyEvent;
use num_traits::ConstZero;
use ratatui::{prelude::*, widgets::*};
use serial_sensors_proto::versions::Version1DataFrame;
use serial_sensors_proto::SensorData;
use tokio::sync::mpsc::UnboundedSender;

use crate::action::Action;
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
                    let (highlight_x, highlight_y, highlight_z) =
                        highlight_axis_3(vec.x, vec.y, vec.z);

                    line.extend(vec![
                        Span::styled("acc", Style::default().cyan()),
                        "  = (".into(),
                        axis_to_span(vec.x as f32 / 16384.0, highlight_x), // TODO: Don't assume normalization
                        ", ".into(),
                        axis_to_span(vec.y as f32 / 16384.0, highlight_y), // TODO: Don't assume normalization
                        ", ".into(),
                        axis_to_span(vec.z as f32 / 16384.0, highlight_z), // TODO: Don't assume normalization
                        ")".into(),
                    ]);
                } else if let SensorData::MagnetometerI16(vec) = frame.value {
                    let (highlight_x, highlight_y, highlight_z) =
                        highlight_axis_3(vec.x, vec.y, vec.z);

                    line.extend(vec![
                        Span::styled("mag", Style::default().cyan()),
                        "  = (".into(),
                        axis_to_span(vec.x as f32 / 1100.0, highlight_x), // TODO: Don't assume normalization
                        ", ".into(),
                        axis_to_span(vec.y as f32 / 1100.0, highlight_y), // TODO: Don't assume normalization
                        ", ".into(),
                        axis_to_span(vec.z as f32 / 1100.0, highlight_z), // TODO: Don't assume normalization
                        ")".into(),
                    ]);
                } else if let SensorData::TemperatureI16(value) = frame.value {
                    line.extend(vec![
                        Span::styled("temp", Style::default().cyan()),
                        " = ".into(),
                        axis_to_span(value.value as f32 / 8.0 + 20.0, false), // TODO: Don't assume normalization
                        "°C".into(),
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

fn axis_to_span<'a, V>(value: V, highlight: bool) -> Span<'a>
where
    V: Display + 'a,
{
    let span = Span::styled(format!("{:+4.6}", value), Style::default());
    if highlight {
        span.green()
    } else {
        span.white()
    }
}

fn highlight_axis_3<T>(x: T, y: T, z: T) -> (bool, bool, bool)
where
    T: PartialOrd + ConstZero + Neg<Output = T>,
{
    let x = if x > T::ZERO { x } else { -x };
    let y = if y > T::ZERO { y } else { -y };
    let z = if z > T::ZERO { z } else { -z };

    if x > y && x > z {
        (true, false, false)
    } else if y > x && y > z {
        (false, true, false)
    } else if z > x && z > y {
        (false, false, true)
    } else {
        (false, false, false)
    }
}
