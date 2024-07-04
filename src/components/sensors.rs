use std::default::Default;
use std::fmt::Display;
use std::ops::Neg;
use std::sync::Arc;

use color_eyre::eyre::Result;
use crossterm::event::KeyEvent;
use num_traits::ConstZero;
use ratatui::{prelude::*, widgets::*};
use tokio::sync::mpsc::UnboundedSender;

use crate::action::Action;
use crate::data_buffer::SensorDataBuffer;

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

    fn handle_key_events(&mut self, _key: KeyEvent) -> Result<Option<Action>> {
        // TODO: Add action to clear the buffer?
        Ok(None)
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
            .map(|id| (id.clone(), self.receiver.get_latest_by_sensor(id)))
            .filter(|(_, frame)| frame.is_some())
            .map(|(id, frame)| (id, frame.expect("value exists")))
            .enumerate()
            .map(|(no, (id, frame))| {
                vec![
                    Span::styled(format!("{no}"), Style::default()),
                    ": ".into(),
                    Span::styled(id.tag().to_string(), Style::default().yellow()),
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
                ]
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

#[allow(dead_code)]
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

#[allow(dead_code)]
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
