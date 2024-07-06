extern crate core;

use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use async_compression::tokio::write::GzipEncoder;
use async_compression::Level;
use clap::Parser;
use color_eyre::eyre::Result;
pub use ratatui::prelude::*;
use serial_sensors_proto::versions::Version1DataFrame;
use serial_sensors_proto::{deserialize, DeserializationError, SensorData, SensorId};
use tokio::fs::File;
use tokio::io::{self, AsyncReadExt, AsyncWriteExt, BufWriter};
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use tokio_serial::{DataBits, FlowControl, Parity, SerialPortBuilderExt, SerialStream, StopBits};

use crate::app::App;
use crate::cli::{Cli, Commands};
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

#[tokio::main(flavor = "multi_thread", worker_threads = 4)]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    initialize_logging()?;
    initialize_panic_handler()?;

    let args = Cli::parse();

    let buffer = Arc::new(SensorDataBuffer::default());

    // Open the serial port
    let port = tokio_serial::new(args.port, args.baud)
        .data_bits(DataBits::Eight)
        .flow_control(FlowControl::None)
        .parity(Parity::None)
        .stop_bits(StopBits::One)
        .timeout(Duration::from_millis(10))
        .open_native_async()
        .expect("Failed to open port");

    let (from_device, receiver) = unbounded_channel::<Vec<u8>>();
    let (frames_tx, frames_rx) = unbounded_channel::<Version1DataFrame>();
    let (_command, to_device) = unbounded_channel::<String>();
    // let (decoder_send, decoded_event) = unbounded_channel::<Version1DataFrame>();

    // Spawn a thread for reading data from the serial port
    tokio::spawn(handle_data_recv(port, from_device, to_device));

    // Run the app.
    match args.command {
        Commands::Ui(args) => {
            // Spawn a decoder thread.
            tokio::spawn(decoder(receiver, frames_tx));

            // Spawn a buffer thread.
            tokio::spawn(decoder_to_buffer(frames_rx, buffer.clone()));

            let mut app = App::new(args.frame_rate, buffer)?;
            app.run().await?;
        }
        Commands::Dump(args) => {
            // Intercept frames when dumping raw data.
            let receiver = if let Some(ref path) = args.raw {
                let gzip = path
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .map(|ext| ext == "gz")
                    .unwrap_or(false);

                let file = match File::create(path).await {
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
            tokio::spawn(decoder(receiver, frames_tx));

            // Process frames.
            dump_data(args.dir, frames_rx).await?;
        }
    }

    Ok(())
}

async fn dump_raw(
    file: File,
    mut rx: UnboundedReceiver<Vec<u8>>,
    tx: UnboundedSender<Vec<u8>>,
) -> Result<()> {
    let mut writer = BufWriter::new(file);
    loop {
        if let Some(data) = rx.recv().await {
            writer.write_all(&data).await?;
            tx.send(data)?;
        }
    }
}

async fn dump_raw_gzipped(
    file: File,
    mut rx: UnboundedReceiver<Vec<u8>>,
    tx: UnboundedSender<Vec<u8>>,
) -> Result<()> {
    let buffered_writer = BufWriter::new(file);
    let mut writer = GzipEncoder::with_quality(buffered_writer, Level::Default);
    loop {
        if let Some(data) = rx.recv().await {
            if let Err(e) = writer.write_all(&data).await {
                writer.flush().await.ok();
                return Err(e.into());
            }
            if let Err(e) = tx.send(data) {
                writer.flush().await.ok();
                return Err(e.into());
            }
        }
    }

    // TODO: Add rendezvous on CTRL-C
}

async fn dump_data(directory: PathBuf, mut rx: UnboundedReceiver<Version1DataFrame>) -> Result<()> {
    let mut files: HashMap<SensorId, BufWriter<File>> = HashMap::new();

    loop {
        let now = SystemTime::now();
        let since_the_epoch = now.duration_since(UNIX_EPOCH).expect("Time went backwards");

        if let Some(data) = rx.recv().await {
            println!("Data received: {:?}", data);
            let target = SensorId::from(&data);

            match files.entry(target.clone()) {
                Entry::Occupied(mut entry) => {
                    let data = create_data_row(since_the_epoch, target, &data);
                    entry.get_mut().write_all(&data).await?;
                }
                Entry::Vacant(entry) => {
                    let file_name = format!(
                        "{}-{}x{}.csv",
                        target.tag(),
                        target.num_components().unwrap_or(0),
                        target.value_type() as u8
                    );
                    println!("New sensor; creating new file: {file_name}");
                    let path = directory.join(file_name);
                    let file = match File::create(path).await {
                        Ok(file) => file,
                        Err(e) => {
                            return Err(e.into());
                        }
                    };

                    let writer = entry.insert(BufWriter::new(file));

                    // Create header row.
                    let header = create_header_row(&data);
                    writer.write_all(&header).await?;

                    // Create first data row.
                    let data = create_data_row(since_the_epoch, target, &data);
                    writer.write_all(&data).await?;

                    writer.flush().await?;
                }
            };
        }
    }
}

fn create_header_row(data: &Version1DataFrame) -> Vec<u8> {
    let mut row = String::from("host_time,sensor_tag,num_components,value_type");
    match data.value {
        SensorData::SystemClockFrequency(_) => {}
        SensorData::AccelerometerI16(_) => {}
        SensorData::MagnetometerI16(_) => {}
        SensorData::TemperatureI16(_) => {}
        SensorData::GyroscopeI16(_) => {}
        SensorData::HeadingI16(_) => {}
        SensorData::EulerAnglesF32(_) => {}
        SensorData::OrientationQuaternionF32(_) => {}
        SensorData::LinearRanges(_) => {}
        SensorData::Identification(_) => {}
    }
    row.push('\n');
    row.as_bytes().into()
}

fn create_data_row(
    since_the_epoch: Duration,
    target: SensorId,
    data: &Version1DataFrame,
) -> Vec<u8> {
    let mut row = format!(
        "{},{:02X},{},{:02X}",
        since_the_epoch.as_secs_f64(),
        target.tag(),
        target.num_components().unwrap_or(0),
        target.value_type() as u8
    );
    match data.value {
        SensorData::SystemClockFrequency(_) => {}
        SensorData::AccelerometerI16(_) => {}
        SensorData::MagnetometerI16(_) => {}
        SensorData::TemperatureI16(_) => {}
        SensorData::GyroscopeI16(_) => {}
        SensorData::HeadingI16(_) => {}
        SensorData::EulerAnglesF32(_) => {}
        SensorData::OrientationQuaternionF32(_) => {}
        SensorData::LinearRanges(_) => {}
        SensorData::Identification(_) => {}
    }
    row.push('\n');
    row.as_bytes().into()
}

async fn decoder(
    mut receiver: UnboundedReceiver<Vec<u8>>,
    sender: UnboundedSender<Version1DataFrame>,
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

async fn decoder_to_buffer(
    mut receiver: UnboundedReceiver<Version1DataFrame>,
    data_buffer: Arc<SensorDataBuffer>,
) -> anyhow::Result<()> {
    loop {
        if let Some(data) = receiver.recv().await {
            data_buffer.enqueue(data);
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
