use serialport::{DataBits, FlowControl, Parity, StopBits};
use std::io::{self, Write};
use std::thread;
use std::time::Duration;

fn main() -> io::Result<()> {
    let port_name = "/dev/ttyACM0";
    let baud_rate = 1_000_000;

    let mut port = serialport::new(port_name, baud_rate)
        .data_bits(DataBits::Eight)
        .stop_bits(StopBits::One)
        .parity(Parity::None)
        .flow_control(FlowControl::None)
        .timeout(Duration::from_millis(10))
        .open()?;

    // Spawn a thread for reading data from the serial port
    let mut reader_port = port.try_clone().expect("Failed to clone port");
    thread::spawn(move || {
        let mut buf: Vec<u8> = vec![0; 1024];
        loop {
            match reader_port.read(&mut buf) {
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
    });

    // Main loop for sending commands to the serial port
    loop {
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        port.write_all(input.as_bytes())?;
        port.flush()?;
    }
}
