#[cfg(any(feature = "dump", feature = "analyze"))]
use std::path::PathBuf;

use clap::{Parser, Subcommand};

use crate::utils::version;

#[derive(Parser, Debug)]
#[command(author, version = version(), about)]
#[command(propagate_version = true)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    #[cfg(feature = "tui")]
    Ui(UiCommand),
    #[cfg(feature = "dump")]
    Dump(Dump),
    #[cfg(feature = "analyze")]
    AnalyzeDump(AnalyzeDump),
}

/// Runs a UI to visualize the incoming data stream.
#[cfg(feature = "tui")]
#[derive(Parser, Debug)]
pub struct UiCommand {
    #[arg(
        short,
        long,
        value_name = "PORT",
        help = "The port name",
        default_value = "/dev/ttyACM0"
    )]
    pub port: String,

    #[arg(
        short,
        long,
        value_name = "BAUD_RATE",
        help = "The baud rate",
        default_value_t = 1_000_000
    )]
    pub baud: u32,

    #[arg(
        short,
        long,
        value_name = "FLOAT",
        help = "Frame rate, i.e. number of frames per second",
        default_value_t = 30.0
    )]
    pub frame_rate: f64,
}

/// Dumps received data to disk.
#[cfg(feature = "dump")]
#[derive(Parser, Debug)]
pub struct Dump {
    #[arg(
        short,
        long,
        value_name = "PORT",
        help = "The port name",
        default_value = "/dev/ttyACM0"
    )]
    pub port: String,

    #[arg(
        short,
        long,
        value_name = "BAUD_RATE",
        help = "The baud rate",
        default_value_t = 1_000_000
    )]
    pub baud: u32,

    #[arg(
        short,
        long,
        value_name = "RAW_FILE",
        help = "The file in which to store raw data"
    )]
    pub raw: Option<PathBuf>,

    #[arg(
        short,
        long,
        value_name = "DIRECTORY",
        help = "The directory in which to store data"
    )]
    pub dir: PathBuf,
}

/// Analyze received data from disk.
#[derive(Parser, Debug)]
#[cfg(feature = "analyze")]
pub struct AnalyzeDump {
    #[arg(
        short,
        long,
        value_name = "DIRECTORY",
        help = "The directory from which to read data"
    )]
    pub dir: PathBuf,

    #[arg(
        short,
        long,
        value_name = "OUTPUT_DIR",
        help = "The output directory to which to store data"
    )]
    pub output: Option<PathBuf>,
}
