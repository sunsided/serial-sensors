use std::time::Duration;
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

    let (serial_in, mut receiver) = unbounded_channel::<String>();
    let (command, serial_out) = unbounded_channel::<String>();

    // Spawn a thread for reading data from the serial port
    tokio::spawn(handle_data_recv(port, serial_in, serial_out));

    // Spawn a task for reading from stdin and sending commands
    tokio::spawn(handle_std_input(command));

    // Main loop for printing input from the serial line.
    loop {
        if let Some(data) = receiver.recv().await {
            print!("{}", data);
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
    serial_in: UnboundedSender<String>,
    mut serial_out: UnboundedReceiver<String>,
) -> anyhow::Result<()> {
    let mut buf: Vec<u8> = vec![0; 1024];
    loop {
        // Send data when serial_out has a message
        if let Ok(command) = serial_out.try_recv() {
            port.write_all(command.as_bytes()).await?;
        }

        match port.read(&mut buf).await {
            Ok(bytes_read) => {
                if bytes_read > 0 {
                    let data = String::from_utf8_lossy(&buf[..bytes_read]).into_owned();
                    serial_in.send(data)?;
                    io::stdout().flush().await?;
                }
            }
            Err(ref e) if e.kind() == io::ErrorKind::TimedOut => (),
            Err(e) => eprintln!("{:?}", e),
        }
    }
}
