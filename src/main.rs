use std::ops::Deref;
use std::sync::Arc;
use std::time::Duration;

use array_pool::pool::{ArrayPool, BorrowingSlice};
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

    let (from_device, mut receiver) = unbounded_channel::<Packet>();
    let (command, to_device) = unbounded_channel::<String>();

    // Pool for sharing data
    let pool: Arc<ArrayPool<u8>> = Arc::new(ArrayPool::new());

    // Spawn a thread for reading data from the serial port
    tokio::spawn(handle_data_recv(port, from_device, to_device, pool.clone()));

    // Spawn a task for reading from stdin and sending commands
    tokio::spawn(handle_std_input(command));

    // Main loop for printing input from the serial line.
    loop {
        if let Some(data) = receiver.recv().await {
            let data = String::from_utf8_lossy(&data).into_owned();
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
    from_device: UnboundedSender<Packet>,
    mut to_device: UnboundedReceiver<String>,
    pool: Arc<ArrayPool<u8>>,
) -> anyhow::Result<()> {
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
                        let mut slice = pool.rent(bytes_read).map_err(|_| anyhow::Error::msg("failed to borrow from pool"))?;
                        slice[..bytes_read].copy_from_slice(&buf[..bytes_read]);
                        from_device.send(Packet::new(slice, bytes_read))?;
                    }
                }
                Err(ref e) if e.kind() == io::ErrorKind::TimedOut => (),
                Err(e) => eprintln!("{:?}", e),
            }
        }
    }
}

struct Packet {
    buffer: BorrowingSlice<u8>,
    size: usize,
}

impl Packet {
    pub fn new(buffer: BorrowingSlice<u8>, size: usize) -> Self {
        Self { buffer, size }
    }
}

impl AsRef<[u8]> for Packet {
    fn as_ref(&self) -> &[u8] {
        &self.buffer[..self.size]
    }
}

impl Deref for Packet {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        self.as_ref()
    }
}
