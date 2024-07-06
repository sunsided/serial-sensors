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
use num_traits::real::Real;
pub use ratatui::prelude::*;
use serial_sensors_proto::types::LinearRangeInfo;
use serial_sensors_proto::versions::Version1DataFrame;
use serial_sensors_proto::{
    deserialize, DataFrame, DeserializationError, IdentifierCode, ScalarData, SensorData, SensorId,
    ValueType, Vector3Data, Vector4Data,
};
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
    let mut ranges: HashMap<SensorId, LinearRangeInfo> = HashMap::new();

    loop {
        let now = SystemTime::now();
        let since_the_epoch = now.duration_since(UNIX_EPOCH).expect("Time went backwards");

        if let Some(data) = rx.recv().await {
            println!("Data received: {:?}", data);
            let target = SensorId::from(&data);
            let sdt = map_data(&data.value);

            let ranges = if let SensorData::LinearRanges(ref info) = data.value {
                ranges.insert(data.target(), info.clone());
                ranges.get(&data.target())
            } else {
                ranges.get(&target.clone())
            };

            let data_row = match create_data_row(since_the_epoch, &target, &data, ranges) {
                None => continue,
                Some(data) => data,
            };

            match files.entry(target.clone()) {
                Entry::Occupied(mut entry) => {
                    entry.get_mut().write_all(&data_row).await?;
                    entry.get_mut().flush().await?;
                }
                Entry::Vacant(entry) => {
                    let file_name = format!(
                        "{}-{}-{}-x{}.csv",
                        target.tag(),
                        sdt.0,
                        value_type_code(target.value_type()),
                        target.num_components().unwrap_or(0)
                    );
                    println!("New sensor; creating new file: {file_name}");
                    let path = directory.join(file_name);
                    let file = match File::create(path).await {
                        Ok(file) => file,
                        Err(e) => {
                            return Err(e.into());
                        }
                    };

                    // Create header row.
                    if let Some(header) = create_header_row(&data) {
                        let writer = entry.insert(BufWriter::new(file));
                        writer.write_all(&header).await?;
                        writer.write_all(&data_row).await?;
                        writer.flush().await?;
                    }
                }
            };
        }
    }
}

#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Hash)]
struct SensorDataType(&'static str);

fn map_data(data: &SensorData) -> SensorDataType {
    match data {
        SensorData::SystemClockFrequency(_) => SensorDataType("clock"),
        SensorData::AccelerometerI16(_) => SensorDataType("acc"),
        SensorData::MagnetometerI16(_) => SensorDataType("mag"),
        SensorData::TemperatureI16(_) => SensorDataType("temp"),
        SensorData::GyroscopeI16(_) => SensorDataType("gyro"),
        SensorData::HeadingI16(_) => SensorDataType("heading"),
        SensorData::EulerAnglesF32(_) => SensorDataType("euler"),
        SensorData::OrientationQuaternionF32(_) => SensorDataType("quat"),
        SensorData::LinearRanges(_) => SensorDataType("lranges"),
        SensorData::Identification(_) => SensorDataType("ident"),
    }
}

fn create_header_row(data: &Version1DataFrame) -> Option<Vec<u8>> {
    let mut row = String::from("host_time,device_time,sensor_tag,num_components,value_type");
    match data.value {
        SensorData::SystemClockFrequency(_) => row.push_str(",freq"),
        SensorData::AccelerometerI16(_) => row.push_str(",x,y,z,converted_x,converted_y,converted_z"),
        SensorData::MagnetometerI16(_) => row.push_str(",x,y,z,converted_x,converted_y,converted_z"),
        SensorData::TemperatureI16(_) => row.push_str(",temp,converted_temp"),
        SensorData::GyroscopeI16(_) => row.push_str(",x,y,z,converted_x,converted_y,converted_z"),
        SensorData::HeadingI16(_) => row.push_str(",heading,converted_heading"),
        SensorData::EulerAnglesF32(_) => row.push_str(",x,y,z,converted_x,converted_y,converted_z"),
        SensorData::OrientationQuaternionF32(_) => row.push_str(",a,b,c,d,converted_a,converted_b,converted_c,converted_d"),
        SensorData::LinearRanges(_) => row.push_str(",resolution_bits,scale_op,scale,scale_raw,scale_decimals,offset,offset_raw,offset_decimals"),
        SensorData::Identification(_) => row.push_str(",code,value"),
    }
    row.push('\n');
    Some(row.as_bytes().into())
}

fn create_data_row(
    since_the_epoch: Duration,
    target: &SensorId,
    data: &Version1DataFrame,
    ranges: Option<&LinearRangeInfo>,
) -> Option<Vec<u8>> {
    let device_time = decode_device_time(data);
    let mut row = format!(
        "{},{},{:02X},{},{},",
        since_the_epoch.as_secs_f64(),
        device_time,
        target.tag(),
        target.num_components().unwrap_or(0),
        value_type_code(target.value_type())
    );
    match data.value {
        SensorData::SystemClockFrequency(data) => row.push_str(&format!("{}", data.value)),
        SensorData::AccelerometerI16(vec) => {
            row.push_str(&format!("{},{},{}", vec.x, vec.y, vec.z));
            csv_convert_push_vec3(&mut row, &vec, &ranges)
        }
        SensorData::MagnetometerI16(vec) => {
            row.push_str(&format!("{},{},{}", vec.x, vec.y, vec.z));
            csv_convert_push_vec3(&mut row, &vec, &ranges)
        }
        SensorData::TemperatureI16(temp) => {
            row.push_str(&format!("{}", temp.value));
            csv_convert_push_scalar(&mut row, &temp, &ranges)
        }
        SensorData::GyroscopeI16(vec) => {
            row.push_str(&format!("{},{},{}", vec.x, vec.y, vec.z));
            csv_convert_push_vec3(&mut row, &vec, &ranges)
        }
        SensorData::HeadingI16(heading) => {
            row.push_str(&format!("{}", heading.value));
            csv_convert_push_scalar(&mut row, &heading, &ranges)
        }
        SensorData::EulerAnglesF32(vec) => {
            row.push_str(&format!("{},{},{}", vec.x, vec.y, vec.z));
            csv_convert_push_vec3(&mut row, &vec, &ranges)
        }
        SensorData::OrientationQuaternionF32(vec) => {
            row.push_str(&format!("{},{},{},{}", vec.a, vec.b, vec.c, vec.d));
            csv_convert_push_vec4(&mut row, &vec, &ranges)
        }
        SensorData::LinearRanges(ref lr) => row.push_str(&format!(
            "{},{:02X},{},{},{},{},{},{}",
            lr.resolution_bits,
            lr.scale_op,
            lr.scale as f32 * 10.0.powi(-(lr.scale_decimals as i32)),
            lr.scale,
            lr.scale_decimals,
            lr.offset as f32 * 10.0.powi(-(lr.offset_decimals as i32)),
            lr.offset,
            lr.offset_decimals
        )),
        SensorData::Identification(ref ident) => row.push_str(&format!(
            "{},{}",
            ident_code(ident.code),
            std::str::from_utf8(&ident.value).unwrap_or("").trim()
        )),
    }
    row.push('\n');
    Some(row.as_bytes().into())
}

fn decode_device_time(data: &Version1DataFrame) -> f32 {
    if data.system_secs != u32::MAX {
        data.system_secs as f32
            + if data.system_millis != u16::MAX {
                data.system_millis as f32 / 1_000.0
            } else {
                0.0
            }
            + if data.system_nanos != u16::MAX {
                data.system_nanos as f32 / 1_000_000.0
            } else {
                0.0
            }
    } else {
        0.0
    }
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

fn csv_convert_push_scalar(
    string: &mut String,
    vec: &ScalarData<i16>,
    ri: &Option<&LinearRangeInfo>,
) {
    if let Some(ri) = ri {
        let x = ri.convert(vec.value as f32);
        string.push_str(&format!(",{}", x))
    } else {
        string.push(',')
    }
}

fn csv_convert_push_vec3<T>(
    string: &mut String,
    vec: &Vector3Data<T>,
    ri: &Option<&LinearRangeInfo>,
) where
    T: Into<f32> + Copy,
{
    if let Some(ri) = ri {
        let x = ri.convert(vec.x.into());
        let y = ri.convert(vec.y.into());
        let z = ri.convert(vec.z.into());
        string.push_str(&format!(",{},{},{}", x, y, z))
    } else {
        string.push_str(",,,")
    }
}

fn csv_convert_push_vec4<T>(
    string: &mut String,
    vec: &Vector4Data<T>,
    ri: &Option<&LinearRangeInfo>,
) where
    T: Into<f32> + Copy,
{
    if let Some(ri) = ri {
        let a = ri.convert(vec.a.into());
        let b = ri.convert(vec.b.into());
        let c = ri.convert(vec.c.into());
        let d = ri.convert(vec.d.into());
        string.push_str(&format!(",{},{},{},{}", a, b, c, d))
    } else {
        string.push_str(",,,,")
    }
}

fn ident_code(code: IdentifierCode) -> &'static str {
    match code {
        IdentifierCode::Generic => "generic",
        IdentifierCode::Maker => "maker",
        IdentifierCode::Product => "product",
        IdentifierCode::Revision => "revision",
    }
}

fn value_type_code(vt: ValueType) -> &'static str {
    match vt {
        ValueType::UInt8 => "u8",
        ValueType::SInt8 => "i8",
        ValueType::UInt16 => "u16",
        ValueType::SInt16 => "i16",
        ValueType::UInt32 => "u32",
        ValueType::SInt32 => "i32",
        ValueType::UInt64 => "u64",
        ValueType::SInt64 => "i64",
        ValueType::UInt128 => "u128",
        ValueType::SInt128 => "i128",
        ValueType::Float32 => "f32",
        ValueType::Float64 => "f64",
        ValueType::Q8_8 => "Q8_8",
        ValueType::Q16_16 => "Q16_16",
        ValueType::Q32_32 => "Q32_32",
        ValueType::LinearRange => "lrange",
        ValueType::Identifier => "ident",
    }
}
