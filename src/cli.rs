use std::path::PathBuf;

use clap::{Parser, Subcommand};

use crate::utils::version;

#[derive(Parser, Debug)]
#[command(author, version = version(), about)]
#[command(propagate_version = true)]
pub struct Cli {
    #[arg(
        global = true,
        short,
        long,
        value_name = "PORT",
        help = "The port name",
        default_value = "/dev/ttyACM0"
    )]
    pub port: String,

    #[arg(
        global = true,
        short,
        long,
        value_name = "BAUD_RATE",
        help = "The baud rate",
        default_value_t = 1_000_000
    )]
    pub baud: u32,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    Ui(UiCommand),
    Dump(Dump),
}

/// Runs a UI to visualize the incoming data stream.
#[derive(Parser, Debug)]
pub struct UiCommand {
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
#[derive(Parser, Debug)]
pub struct Dump {
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
