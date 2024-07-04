extern crate core;
use color_eyre::eyre::Result;
use ratatui::crossterm::ExecutableCommand;
use ratatui::prelude::*;
use serial_sensors_proto::{DeserializationError, deserialize, SensorData};
use tokio::io::{self, AsyncBufReadExt, AsyncReadExt, AsyncWriteExt};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio_serial::{SerialPortBuilderExt, SerialStream};

use crate::utils::initialize_logging;

mod cli;
mod tui;
mod utils;

const PORT_NAME: &str = "/dev/ttyACM1";
const BAUD_RATE: u32 = 1_000_000;

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    initialize_logging()?;

    log::info!("cool");

    /*
    // Setup terminal
    let mut stdout = std::io::stdout();
    stdout.execute(EnterAlternateScreen)?;
    enable_raw_mode()?;

    let mut terminal = Terminal::new(CrosstermBackend::new(stdout))?;
    terminal.clear()?;

    loop {
        terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .margin(1)
                .constraints(
                    [
                        Constraint::Percentage(20),
                        Constraint::Percentage(30),
                        Constraint::Percentage(50),
                    ]
                    .as_ref(),
                )
                .split(f.size());

            /*
            let sensor_data = sensor_data.lock().unwrap();
            let sensor_rows: Vec<ListItem> = sensor_data
                .iter()
                .map(|d| ListItem::new(Spans::from(vec![d.clone()])))
                .collect();
            */

            let sensor_rows = [ListItem::new(Span::from("sensor data 1"))];
            let sensor_list = List::new(sensor_rows)
                .block(Block::default().borders(Borders::ALL).title("Sensor Data"));
            f.render_widget(sensor_list, chunks[0]);

            // let streaming_data = streaming_data.lock().unwrap();

            let streaming_data = ["a lot of text", "really"];
            let streaming_paragraph = Paragraph::new(streaming_data.join("\n")).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Streaming Data"),
            );
            f.render_widget(streaming_paragraph, chunks[1]);

            let data_points = [(1.234, 4.567)];

            let datasets = vec![Dataset::default()
                .name("data 1")
                .marker(symbols::Marker::Dot)
                .graph_type(GraphType::Scatter)
                .style(Style::default().cyan())
                .data(&data_points)];

            let data_chart = Chart::new(datasets)
                .block(Block::default().borders(Borders::ALL).title("Data Chart"))
                .x_axis(ratatui::widgets::Axis::default().bounds([0.0, 10.0]))
                .y_axis(ratatui::widgets::Axis::default().bounds([0.0, 100.0]));
            f.render_widget(data_chart, chunks[2]);
        })?;

        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.code == KeyCode::Char('q') {
                    break;
                }
            }
        }
    }

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

    // Spawn a thread for reading data from the serial port
    let cdc_handle = tokio::spawn(handle_data_recv(port, from_device, to_device));

    // Handle the display of the data.
    let stdout_handle = tokio::spawn(process_terminal(receiver));

    tokio::select! {
        result = cdc_handle => {
            match result {
                Ok(result) => match result {
                    Ok(_) => println!("CDC task completed successfully"),
                    Err(e) => eprintln!("CDC task returned an error: {}", e),
                },
                Err(e) => eprintln!("CDC task panicked: {}", e),
            }
        },
        result = stdout_handle => {
            match result {
                Ok(_) => {},
                Err(e) => eprintln!("Standard output panicked: {}", e),
            }
        }
    };

    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture,
    )?;
    disable_raw_mode()?;
    terminal.show_cursor()?;
    */

    Ok(())
}

async fn process_terminal(mut receiver: UnboundedReceiver<Vec<u8>>) {
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
                                "acc = ({:.04}, {:.04}, {:.04})",
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
