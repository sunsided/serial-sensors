use std::io::{self, Write};
use std::time::Duration;
use tokio::io::AsyncReadExt;
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

    // Spawn a thread for reading data from the serial port
    tokio::spawn(handle_data_recv(port));

    /*
    // Main loop for sending commands to the serial port
    loop {
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        port.write_all(input.as_bytes())?;
        port.flush()?;
    }
     */

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    Ok(())
}

async fn handle_data_recv(mut port: SerialStream) {
    let mut buf: Vec<u8> = vec![0; 1024];
    loop {
        match port.read(&mut buf).await {
            Ok(bytes_read) => {
                if bytes_read > 0 {
                    print!("{}", String::from_utf8_lossy(&buf[..bytes_read]));
                    io::stdout().flush().unwrap();
                }
            }
            Err(ref e) if e.kind() == io::ErrorKind::TimedOut => (),
            Err(e) => eprintln!("{:?}", e),
        }
    }
}
