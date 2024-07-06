use std::sync::Arc;

use color_eyre::eyre::Result;
use ratatui::{prelude::*, widgets::*};

use crate::action::Action;
use crate::data_buffer::SensorDataBuffer;

use super::Component;

#[derive(Clone)]
pub struct FpsDisplay {
    receiver: Arc<SensorDataBuffer>,
}

impl FpsDisplay {
    pub fn new(receiver: Arc<SensorDataBuffer>) -> Self {
        Self { receiver }
    }
}

impl Component for FpsDisplay {
    fn update(&mut self, _action: Action) -> Result<Option<Action>> {
        Ok(None)
    }

    fn draw(&mut self, f: &mut Frame<'_>, rect: Rect) -> Result<()> {
        let durations = self.receiver.average_duration();
        let fps = durations.as_secs_f32().recip();

        let num_sensors = self.receiver.num_sensors();

        let rects = Layout::default()
            .direction(Direction::Vertical)
            .constraints(vec![
                Constraint::Length(10), // first row
                Constraint::Min(0),
            ])
            .split(rect);

        let rect = rects[1];

        let s = if num_sensors != 1 { "s" } else { "" };

        let s = format!("{:.2} Hz ({num_sensors} sensor{s})", fps);
        let block = Block::default().title(block::Title::from(s.dim()).alignment(Alignment::Right));
        f.render_widget(block, rect);
        Ok(())
    }
}
