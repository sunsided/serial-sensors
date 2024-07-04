use std::sync::Arc;

use color_eyre::eyre::Result;
use crossterm::event::KeyEvent;
use ratatui::prelude::Rect;
use serde::{Deserialize, Serialize};
use serial_sensors_proto::types::AccelerometerI16;
use serial_sensors_proto::Vector3Data;
use serial_sensors_proto::versions::Version1DataFrame;
use tokio::sync::mpsc;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

use crate::{
    action::Action,
    components::{Component, fps::FpsCounter, home::Home},
    config::Config,
    tui,
};
use crate::components::streaming::StreamingLog;
use crate::data_buffer::SensorDataBuffer;
use crate::tui::Tui;

#[derive(Default, Debug, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Mode {
    #[default]
    Home,
}

pub struct App {
    pub config: Config,
    pub tick_rate: f64,
    pub frame_rate: f64,
    pub components: Vec<Box<dyn Component>>,
    pub should_quit: bool,
    pub should_suspend: bool,
    pub mode: Mode,
    pub last_tick_key_events: Vec<KeyEvent>,
    pub receiver: Arc<SensorDataBuffer>,
}

impl App {
    pub fn new(tick_rate: f64, frame_rate: f64, receiver: Arc<SensorDataBuffer>) -> Result<Self> {
        // let home = Home::new();
        let streaming = StreamingLog::new(receiver.clone());
        let fps = FpsCounter::new();
        let config = Config::new()?;
        let mode = Mode::Home;
        Ok(Self {
            tick_rate,
            frame_rate,
            components: vec![Box::new(streaming), Box::new(fps)],
            should_quit: false,
            should_suspend: false,
            config,
            mode,
            last_tick_key_events: Vec::new(),
            receiver,
        })
    }

    pub async fn run(&mut self) -> Result<()> {
        let (action_tx, mut action_rx) = mpsc::unbounded_channel();

        let mut tui = Tui::new()?;
        tui.tick_rate(self.tick_rate);
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

        // Put some data in.
        action_tx.send(Action::SensorRow(Version1DataFrame::new(
            0,
            1,
            2,
            AccelerometerI16::new(Vector3Data::new(0, 1, 2)),
        )))?;

        loop {
            if let Some(e) = tui.next().await {
                match e {
                    tui::Event::Quit => action_tx.send(Action::Quit)?,
                    tui::Event::Tick => action_tx.send(Action::Tick)?,
                    tui::Event::Render => action_tx.send(Action::Render)?,
                    tui::Event::Resize(x, y) => action_tx.send(Action::Resize(x, y))?,
                    tui::Event::Key(key) => {
                        if let Some(keymap) = self.config.keybindings.get(&self.mode) {
                            if let Some(action) = keymap.get(&vec![key]) {
                                log::info!("Got action: {action:?}");
                                action_tx.send(action.clone())?;
                            } else {
                                // If the key was not handled as a single key action,
                                // then consider it for multi-key combinations.
                                self.last_tick_key_events.push(key);

                                // Check for multi-key combinations
                                if let Some(action) = keymap.get(&self.last_tick_key_events) {
                                    log::info!("Got action: {action:?}");
                                    action_tx.send(action.clone())?;
                                }
                            }
                        };
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
                tui.tick_rate(self.tick_rate);
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
