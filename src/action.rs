use serde::{Deserialize, Serialize};
use serial_sensors_proto::versions::Version1DataFrame;
use strum::Display;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Display)]
pub enum Action {
    Tick,
    Render,
    Resize(u16, u16),
    Suspend,
    Resume,
    Quit,
    Refresh,
    Error(String),
    Help,
    ToggleShowHelp,
    ScheduleIncrement,
    ScheduleDecrement,
    Increment(usize),
    Decrement(usize),
    CompleteInput(String),
    #[serde(skip)]
    SensorRow(Version1DataFrame),
    EnterNormal,
    EnterInsert,
    EnterProcessing,
    ExitProcessing,
    Update,
}
