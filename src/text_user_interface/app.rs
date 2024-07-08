use std::sync::Arc;

use color_eyre::eyre::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::prelude::Rect;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tokio::sync::mpsc::UnboundedSender;

use super::components::sensors::Sensors;
use super::components::streaming::StreamingLog;
use super::data_buffer::SensorDataBuffer;
use super::tui::Tui;
use super::{
    action::Action,
    components::{fps::FpsDisplay, Component},
    config::Config,
    tui,
};

#[derive(Default, Debug, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Mode {
    #[default]
    Home,
}

pub struct App {
    pub config: Config,
    pub frame_rate: f64,
    pub components: Vec<Box<dyn Component>>,
    pub should_quit: bool,
    pub should_suspend: bool,
    pub should_pause: bool,
    pub last_tick_key_events: Vec<KeyEvent>,
}

impl App {
    pub fn new(frame_rate: f64, receiver: Arc<SensorDataBuffer>) -> Result<Self> {
        let sensors = Sensors::new(receiver.clone());
        let streaming = StreamingLog::new(receiver.clone());
        let fps = FpsDisplay::new(receiver);
        let config = Config::new()?;

        Ok(Self {
            frame_rate,
            components: vec![Box::new(sensors), Box::new(streaming), Box::new(fps)],
            should_quit: false,
            should_suspend: false,
            should_pause: false,
            config,
            last_tick_key_events: Vec::new(),
        })
    }

    pub async fn run(&mut self) -> Result<()> {
        let (action_tx, mut action_rx) = mpsc::unbounded_channel();

        let mut tui = Tui::new()?;
        tui.frame_rate(self.frame_rate);
        tui.enter()?;

        for component in self.components.iter_mut() {
            component.register_action_handler(action_tx.clone())?;
        }

        for component in self.components.iter_mut() {
            component.register_config_handler(self.config.clone())?;
        }

        for component in self.components.iter_mut() {
            component.init()?;
        }

        loop {
            if let Some(e) = tui.next().await {
                match e {
                    tui::Event::Quit => action_tx.send(Action::Quit)?,
                    tui::Event::Tick => action_tx.send(Action::Tick)?,
                    tui::Event::Render => action_tx.send(Action::Render)?,
                    tui::Event::Resize(x, y) => action_tx.send(Action::Resize(x, y))?,
                    tui::Event::Key(key) => {
                        if key == KeyEvent::from(KeyCode::Char('q'))
                            || key == KeyEvent::from(KeyCode::Esc)
                            || key == KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)
                        {
                            action_tx.send(Action::Quit)?;
                        } else if key == KeyEvent::from(KeyCode::Pause)
                            || key == KeyEvent::from(KeyCode::Char(' '))
                        {
                            if self.should_pause {
                                action_tx.send(Action::Unpause)?;
                            } else {
                                action_tx.send(Action::Pause)?;
                            }
                        }
                    }
                    _ => {}
                }
                for component in self.components.iter_mut() {
                    if let Some(action) = component.handle_events(Some(e.clone()))? {
                        action_tx.send(action)?;
                    }
                }
            }

            while let Ok(action) = action_rx.try_recv() {
                if action != Action::Tick && action != Action::Render {
                    log::debug!("{action:?}");
                }

                match action {
                    Action::Tick => {
                        self.last_tick_key_events.drain(..);
                    }
                    Action::Quit => self.should_quit = true,
                    Action::Suspend => self.should_suspend = true,
                    Action::Resume => self.should_suspend = false,
                    Action::Pause => self.should_pause = true,
                    Action::Unpause => self.should_pause = false,
                    Action::Resize(w, h) => {
                        tui.resize(Rect::new(0, 0, w, h))?;
                        self.draw_components(&action_tx, &mut tui)?;
                    }
                    Action::Render => {
                        self.draw_components(&action_tx, &mut tui)?;
                    }
                    _ => {}
                }

                for component in self.components.iter_mut() {
                    if let Some(action) = component.update(action.clone())? {
                        action_tx.send(action)?
                    };
                }
            }

            if self.should_suspend {
                tui.suspend()?;
                action_tx.send(Action::Resume)?;
                tui = Tui::new()?;
                tui.frame_rate(self.frame_rate);
                tui.enter()?;
            } else if self.should_quit {
                tui.stop()?;
                break;
            }
        }
        tui.exit()?;
        Ok(())
    }

    fn draw_components(
        &mut self,
        action_tx: &UnboundedSender<Action>,
        tui: &mut Tui,
    ) -> Result<()> {
        tui.draw(|f| {
            for component in self.components.iter_mut() {
                let r = component.draw(f, f.size());
                if let Err(e) = r {
                    action_tx
                        .send(Action::Error(format!("Failed to draw: {:?}", e)))
                        .unwrap();
                }
            }
        })?;
        Ok(())
    }
}
