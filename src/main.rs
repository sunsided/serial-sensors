extern crate core;

#[cfg(feature = "tui")]
use std::sync::Arc;

use clap::Parser;
use color_eyre::eyre::Result;
#[cfg(feature = "tui")]
pub use ratatui::prelude::*;
#[cfg(feature = "serial")]
use serial_sensors_proto::versions::Version1DataFrame;
#[cfg(feature = "serial")]
use tokio::sync::mpsc::unbounded_channel;

#[cfg(feature = "tui")]
use crate::app::App;
use crate::cli::{Cli, Commands};
#[cfg(feature = "tui")]
use crate::data_buffer::SensorDataBuffer;
#[cfg(feature = "dump")]
use crate::dumping::{dump_data, dump_raw, dump_raw_gzipped};
use crate::utils::initialize_logging;

#[cfg(feature = "tui")]
mod action;
#[cfg(feature = "analyze")]
mod analyze;
#[cfg(feature = "tui")]
mod app;
mod cli;
#[cfg(feature = "tui")]
mod components;
#[cfg(feature = "tui")]
mod config;
#[cfg(feature = "tui")]
mod data_buffer;
#[cfg(feature = "dump")]
mod dumping;
#[cfg(feature = "tui")]
mod fps_counter;
#[cfg(feature = "serial")]
mod serial;
#[cfg(feature = "tui")]
mod tui;
mod utils;

#[tokio::main(flavor = "multi_thread", worker_threads = 4)]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    initialize_logging()?;

    #[cfg(feature = "tui")]
    utils::initialize_panic_handler()?;

    let args = Cli::parse();

    // Run the app.
    match args.command {
        #[cfg(feature = "tui")]
        Commands::Ui(args) => {
            let (from_device, receiver) = unbounded_channel::<Vec<u8>>();

            let (_command, to_device) = unbounded_channel::<String>();
            serial::start_receive(from_device, to_device, &args.port, args.baud);

            // Spawn a decoder thread.
            let (frames_tx, frames_rx) = unbounded_channel::<Version1DataFrame>();
            tokio::spawn(serial::decoder(receiver, frames_tx));

            // Spawn a buffer thread.
            let buffer = Arc::new(SensorDataBuffer::default());
            tokio::spawn(serial::decoder_to_buffer(frames_rx, buffer.clone()));

            let mut app = App::new(args.frame_rate, buffer)?;
            app.run().await?;
        }
        #[cfg(feature = "dump")]
        Commands::Dump(args) => {
            let (from_device, receiver) = unbounded_channel::<Vec<u8>>();
            let (_command, to_device) = unbounded_channel::<String>();
            serial::start_receive(from_device, to_device, &args.port, args.baud);

            // Intercept frames when dumping raw data.
            let receiver = if let Some(ref path) = args.raw {
                let gzip = path
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .map(|ext| ext == "gz")
                    .unwrap_or(false);

                let file = match tokio::fs::File::create(path).await {
                    Ok(file) => file,
                    Err(e) => {
                        return Err(e.into());
                    }
                };

                let (tx, raw_rx) = unbounded_channel();
                if gzip {
                    tokio::spawn(dump_raw_gzipped(file, receiver, tx));
                } else {
                    tokio::spawn(dump_raw(file, receiver, tx));
                }
                raw_rx
            } else {
                receiver
            };

            // Spawn a decoder thread.
            let (frames_tx, frames_rx) = unbounded_channel::<Version1DataFrame>();
            tokio::spawn(serial::decoder(receiver, frames_tx));

            // Process frames.
            dump_data(args.dir, frames_rx).await?;
        }
        #[cfg(feature = "analyze")]
        Commands::AnalyzeDump(args) => {
            let output = args.output.unwrap_or(args.dir.clone());
            analyze::analyze_dump(args.dir, output)?;
        }
    }

    Ok(())
}
