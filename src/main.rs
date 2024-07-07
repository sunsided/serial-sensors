extern crate core;

#[cfg(feature = "tui")]
use std::sync::Arc;
#[cfg(feature = "serial")]
use std::time::Duration;

use clap::Parser;
use color_eyre::eyre::Result;
#[cfg(feature = "tui")]
pub use ratatui::prelude::*;
#[cfg(feature = "serial")]
use serial_sensors_proto::{deserialize, versions::Version1DataFrame, DeserializationError};
#[cfg(feature = "serial")]
use tokio::io::{self, AsyncReadExt, AsyncWriteExt};
#[cfg(feature = "serial")]
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
#[cfg(feature = "serial")]
use tokio_serial::{DataBits, FlowControl, Parity, SerialPortBuilderExt, SerialStream, StopBits};

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
            start_receive(from_device, to_device, &args.port, args.baud);

            // Spawn a decoder thread.
            let (frames_tx, frames_rx) = unbounded_channel::<Version1DataFrame>();
            tokio::spawn(decoder(receiver, frames_tx));

            // Spawn a buffer thread.
            let buffer = Arc::new(SensorDataBuffer::default());
            tokio::spawn(decoder_to_buffer(frames_rx, buffer.clone()));

            let mut app = App::new(args.frame_rate, buffer)?;
            app.run().await?;
        }
        #[cfg(feature = "dump")]
        Commands::Dump(args) => {
            let (from_device, receiver) = unbounded_channel::<Vec<u8>>();
            let (_command, to_device) = unbounded_channel::<String>();
            start_receive(from_device, to_device, &args.port, args.baud);

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
            tokio::spawn(decoder(receiver, frames_tx));

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

#[cfg(feature = "serial")]
fn start_receive(
    from_device: UnboundedSender<Vec<u8>>,
    to_device: UnboundedReceiver<String>,
    port: &str,
    baud_rate: u32,
) {
    // Open the serial port
    let port = tokio_serial::new(port, baud_rate)
        .data_bits(DataBits::Eight)
        .flow_control(FlowControl::None)
        .parity(Parity::None)
        .stop_bits(StopBits::One)
        .timeout(Duration::from_millis(10))
        .open_native_async()
        .expect("Failed to open port");

    // Spawn a thread for reading data from the serial port
    tokio::spawn(handle_data_recv(port, from_device, to_device));
}

#[cfg(feature = "serial")]
async fn decoder(
    mut receiver: UnboundedReceiver<Vec<u8>>,
    sender: UnboundedSender<Version1DataFrame>,
) -> Result<()> {
    // Main loop for printing input from the serial line.
    let mut buffer = Vec::with_capacity(1024);
    loop {
        if let Some(data) = receiver.recv().await {
            // Double buffer the data because we may need to restart reading.
            buffer.extend_from_slice(&data);

            match deserialize(&mut buffer) {
                Ok((read, frame)) => {
                    // Remove all ready bytes.
                    buffer.drain(0..read);

                    // Ensure that we don't keep delimiter bytes in the buffer.
                    let first_nonzero = buffer.iter().position(|&x| x != 0).unwrap_or(buffer.len());
                    buffer.drain(0..first_nonzero);

                    sender.send(frame.data)?;
                }
                Err(e) => {
                    match e {
                        DeserializationError::Truncated => {
                            // ignored; this is a synchronization issue
                            log::warn!("Received data was truncated");
                        }
                        DeserializationError::Corrupt => {
                            // ignored
                            log::error!("Received data was corrupt");
                        }
                        DeserializationError::BincodeError(e) => {
                            log::error!("Binary coding error detected: {e}");
                            buffer.clear();
                        }
                    }
                }
            }
        }
    }
}

#[cfg(feature = "tui")]
async fn decoder_to_buffer(
    mut receiver: UnboundedReceiver<Version1DataFrame>,
    data_buffer: Arc<SensorDataBuffer>,
) -> Result<()> {
    loop {
        if let Some(data) = receiver.recv().await {
            data_buffer.enqueue(data);
        }
    }
}

#[cfg(feature = "serial")]
async fn handle_data_recv(
    mut port: SerialStream,
    from_device: UnboundedSender<Vec<u8>>,
    mut to_device: UnboundedReceiver<String>,
) -> Result<()> {
    let _guard = RecvObserver;
    let mut buf: Vec<u8> = vec![0; 1024];
    loop {
        tokio::select! {
            // Send data when serial_out has a message
            Some(command) = to_device.recv() => {
                port.write_all(command.as_bytes()).await?;
            }

            // Read data from the serial port
            result = port.read(&mut buf) => match result {
                Ok(bytes_read) => {
                    if bytes_read > 0 {
                        let vec = Vec::from(&buf[..bytes_read]);
                        from_device.send(vec)?;
                    }
                }
                Err(ref e) if e.kind() == io::ErrorKind::TimedOut => (),
                Err(e) => eprintln!("{:?}", e),
            }
        }
    }
}

#[cfg(feature = "serial")]
struct RecvObserver;

#[cfg(feature = "serial")]
impl Drop for RecvObserver {
    fn drop(&mut self) {
        println!("Receive loop finished");
    }
}
