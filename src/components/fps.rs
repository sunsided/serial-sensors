use std::sync::Arc;
use std::time::Instant;

use color_eyre::eyre::Result;
use ratatui::{prelude::*, widgets::*};

use crate::action::Action;
use crate::data_buffer::SensorDataBuffer;

use super::Component;

#[derive(Debug, Clone, PartialEq)]
pub enum Ticker {
    AppTick,
    RenderTick,
}

#[derive(Debug, Clone)]
pub struct FpsDisplay {
    render_start_time: Instant,
    render_frames: u32,
    render_fps: f64,

    receiver: Arc<SensorDataBuffer>,
}

impl FpsDisplay {
    pub fn new(receiver: Arc<SensorDataBuffer>) -> Self {
        Self {
            render_start_time: Instant::now(),
            render_frames: 0,
            render_fps: 0.0,
            receiver,
        }
    }

    fn render_tick(&mut self) -> Result<()> {
        self.render_frames += 1;
        let now = Instant::now();
        let elapsed = (now - self.render_start_time).as_secs_f64();
        if elapsed >= 1.0 {
            self.render_fps = self.render_frames as f64 / elapsed;
            self.render_start_time = now;
            self.render_frames = 0;
        }
        Ok(())
    }
}

impl Component for FpsDisplay {
    fn update(&mut self, action: Action) -> Result<Option<Action>> {
        if let Action::Render = action {
            self.render_tick()?
        };
        Ok(None)
    }

    fn draw(&mut self, f: &mut Frame<'_>, rect: Rect) -> Result<()> {
        let durations = self.receiver.average_duration();
        let fps = durations.as_secs_f32().recip();

        let rects = Layout::default()
            .direction(Direction::Vertical)
            .constraints(vec![
                Constraint::Length(1), // first row
                Constraint::Min(0),
            ])
            .split(rect);

        let rect = rects[0];

        let s = format!("{:.2} Hz (data) {:.2} fps (render)", fps, self.render_fps);
        let block = Block::default().title(block::Title::from(s.dim()).alignment(Alignment::Right));
        f.render_widget(block, rect);
        Ok(())
    }
}
