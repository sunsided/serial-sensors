extern crate core;
use std::time::Duration;

use rand_distr::{Distribution, Normal};
use rtplot::{Figure, PlotType};
use serial_sensors_proto::{deserialize, DeserializationError, SensorData};
use tokio::io::{self, AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use tokio_serial::{DataBits, FlowControl, Parity, SerialPortBuilderExt, SerialStream, StopBits};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let port_name = "/dev/ttyACM0";
    let baud_rate = 1_000_000;

    // Open the serial port
    let port = tokio_serial::new(port_name, baud_rate)
        .data_bits(DataBits::Eight)
        .flow_control(FlowControl::None)
        .parity(Parity::None)
        .stop_bits(StopBits::One)
        .timeout(Duration::from_millis(10))
        .open_native_async()
        .expect("Failed to open port");

    let (from_device, receiver) = unbounded_channel::<Vec<u8>>();
    let (command, to_device) = unbounded_channel::<String>();

    let mut figure = Figure::new(100)
        .ylim([-1.0, 1.0])
        .xlabel("Time (s)")
        .ylabel("Amplitude")
        .plot_type(PlotType::Line)
        .color(0x80, 0x00, 0x80);

    let normal = Normal::new(0.0, 1.0).unwrap();
    let mut rng = rand::thread_rng();

    Figure::display(&mut figure, |fig| {
        let v: Vec<f32> = normal
            .sample_iter(&mut rng)
            .take(10)
            .map(|x| x as f32)
            .collect();
        fig.plot_stream(&v);
    });

    // Spawn a thread for reading data from the serial port
    let cdc_handle = tokio::spawn(handle_data_recv(port, from_device, to_device));

    // Spawn a task for reading from stdin and sending commands
    let stdin_handle = tokio::spawn(handle_std_input(command));

    let stdout_handle = tokio::spawn(process_incoming_data(receiver));

    tokio::select! {
        result1 = cdc_handle => {
            match result1 {
                Ok(result) => match result {
                    Ok(_) => println!("CDC task completed successfully"),
                    Err(e) => eprintln!("CDC task returned an error: {}", e),
                },
                Err(e) => eprintln!("CDC task panicked: {}", e),
            }
        },
        result2 = stdin_handle => {
            match result2 {
                Ok(_) => {},
                Err(e) => eprintln!("Standard input panicked: {}", e),
            }
        },
        result2 = stdout_handle => {
            match result2 {
                Ok(_) => {},
                Err(e) => eprintln!("Standard output panicked: {}", e),
            }
        }
    };

    Ok(())
}

async fn process_incoming_data(mut receiver: UnboundedReceiver<Vec<u8>>) {
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

                    print!(
                        "In: {}, {}:{} {:02X}:{:02X} ",
                        frame.data.global_sequence,
                        frame.data.sensor_tag,
                        frame.data.sensor_sequence,
                        frame.data.value.sensor_type_id(),
                        frame.data.value.value_type() as u8,
                    );

                    match frame.data.value {
                        SensorData::AccelerometerI16(vec) => {
                            println!(
                                "acc = ({}, {}, {})",
                                vec.x as f32 / 16384.0,
                                vec.y as f32 / 16384.0,
                                vec.z as f32 / 16384.0
                            )
                        }
                        SensorData::MagnetometerI16(vec) => {
                            println!("mag = ({}, {}, {})", vec.x, vec.y, vec.z)
                        }
                        SensorData::TemperatureI16(value) => {
                            println!("temp = {} °C", value.value as f32 / 8.0 + 20.0)
                        }
                        other => eprintln!("{other:?}"),
                    }
                }
                Err(e) => {
                    match e {
                        DeserializationError::Truncated => {
                            // ignored; this is a synchronization issue
                            eprintln!("truncated");
                        }
                        DeserializationError::Corrupt => {
                            // ignored
                            eprintln!("corrupt");
                        }
                        DeserializationError::BincodeError(e) => {
                            eprintln!("Binary coding error: {e}");
                            buffer.clear();
                        }
                    }
                }
            }
        }
    }
}

async fn handle_std_input(command: UnboundedSender<String>) {
    let stdin = io::stdin();
    let reader = BufReader::new(stdin);
    let mut lines = reader.lines();

    while let Some(line) = lines.next_line().await.unwrap_or(None) {
        let line = line.trim().to_string();
        if !line.is_empty() {
            command.send(line).unwrap();
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
