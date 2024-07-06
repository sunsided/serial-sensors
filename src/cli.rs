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
}

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
