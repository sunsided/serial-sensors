extern crate core;

use std::sync::Arc;
use std::time::Duration;

use clap::Parser;
use color_eyre::eyre::Result;
pub use ratatui::prelude::*;
use serial_sensors_proto::{deserialize, DeserializationError};
use tokio::io::{self, AsyncReadExt, AsyncWriteExt};
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use tokio_serial::{DataBits, FlowControl, Parity, SerialPortBuilderExt, SerialStream, StopBits};

use crate::app::App;
use crate::cli::Cli;
use crate::data_buffer::SensorDataBuffer;
use crate::utils::{initialize_logging, initialize_panic_handler};

mod action;
mod app;
mod cli;
mod components;
mod config;
mod data_buffer;
mod fps_counter;
mod tui;
mod utils;

const PORT_NAME: &str = "/dev/ttyACM0";
const BAUD_RATE: u32 = 1_000_000;

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    initialize_logging()?;
    initialize_panic_handler()?;

    let args = Cli::parse();

    let buffer = Arc::new(SensorDataBuffer::default());

    // Open the serial port
    let port = tokio_serial::new(PORT_NAME, BAUD_RATE)
        .data_bits(DataBits::Eight)
        .flow_control(FlowControl::None)
        .parity(Parity::None)
        .stop_bits(StopBits::One)
        .timeout(Duration::from_millis(10))
        .open_native_async()
        .expect("Failed to open port");

    let (from_device, receiver) = unbounded_channel::<Vec<u8>>();
    let (_command, to_device) = unbounded_channel::<String>();
    // let (decoder_send, decoded_event) = unbounded_channel::<Version1DataFrame>();

    // Spawn a decoder thread.
    tokio::spawn(decoder(receiver, buffer.clone()));

    // Spawn a thread for reading data from the serial port
    tokio::spawn(handle_data_recv(port, from_device, to_device));

    // Run the app.
    let mut app = App::new(args.frame_rate, buffer)?;
    app.run().await?;

    Ok(())
}

async fn decoder(
    mut receiver: UnboundedReceiver<Vec<u8>>,
    data_buffer: Arc<SensorDataBuffer>,
) -> anyhow::Result<()> {
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

                    data_buffer.enqueue(frame.data);
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

async fn handle_data_recv(
    mut port: SerialStream,
    from_device: UnboundedSender<Vec<u8>>,
    mut to_device: UnboundedReceiver<String>,
) -> anyhow::Result<()> {
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

struct RecvObserver;

impl Drop for RecvObserver {
    fn drop(&mut self) {
        println!("Receive loop finished");
    }
}
