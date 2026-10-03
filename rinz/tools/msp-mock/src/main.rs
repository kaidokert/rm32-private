//! A deliberately small MSPv1-over-TCP endpoint for exercising a configurator's
//! manual TCP connection. It is not a flight-controller emulator.

use std::{
    borrow::Cow,
    env,
    io::{self, Read, Write},
    net::{TcpListener, TcpStream},
    thread,
};

use multiwii_serial_protocol::{MspPacket, MspPacketDirection, MspParser};

const DEFAULT_ADDR: &str = "127.0.0.1:5761";

fn main() -> io::Result<()> {
    let address = env::args()
        .nth(1)
        .unwrap_or_else(|| DEFAULT_ADDR.to_owned());
    let listener = TcpListener::bind(&address)?;

    println!("MSP mock listening on tcp://{address}");
    println!("Use that address in Betaflight Configurator's Manual Selection.");

    for connection in listener.incoming() {
        match connection {
            Ok(stream) => {
                let peer = stream.peer_addr().ok();
                println!("client connected: {peer:?}");
                thread::spawn(move || {
                    if let Err(error) = serve(stream) {
                        eprintln!("client {peer:?}: {error}");
                    }
                });
            }
            Err(error) => eprintln!("accept failed: {error}"),
        }
    }

    Ok(())
}

fn serve(mut stream: TcpStream) -> io::Result<()> {
    let mut parser = MspParser::new();
    let mut buffer = [0_u8; 512];

    loop {
        let count = stream.read(&mut buffer)?;
        if count == 0 {
            return Ok(());
        }

        for byte in &buffer[..count] {
            match parser.parse(*byte) {
                Ok(Some(packet)) => {
                    println!(
                        "MSP request: cmd={} payload={} bytes",
                        packet.cmd,
                        packet.data.len()
                    );
                    let response = response_for(packet.cmd);
                    write_packet(&mut stream, packet.cmd, &response)?;
                }
                Ok(None) => {}
                Err(error) => eprintln!("discarded malformed MSP frame: {error:?}"),
            }
        }
    }
}

fn response_for(command: u8) -> Vec<u8> {
    match command {
        // These establish the identity used during an MSP configurator handshake.
        1 => vec![0, 1, 46],   // MSP_API_VERSION
        2 => b"BTFL".to_vec(), // MSP_FC_VARIANT
        3 => vec![4, 5, 0],    // MSP_FC_VERSION
        4 => board_info(),     // MSP_BOARD_INFO
        5 => [
            b"Jun 14 2026".as_slice(),
            b"12:00:00".as_slice(),
            b"mspmock".as_slice(),
        ]
        .concat(),
        10 => b"MSP mock".to_vec(),          // MSP_NAME
        101 => vec![0; 11],                  // MSP_STATUS, safe idle defaults
        150 => vec![0; 16],                  // MSP_STATUS_EX, safe idle defaults
        160 => vec![0x4d, 0x53, 0x50, 0x01], // MSP_UID
        _ => Vec::new(),
    }
}

fn board_info() -> Vec<u8> {
    let mut response = Vec::new();
    response.extend_from_slice(b"MSP0"); // board identifier
    response.extend_from_slice(&0_u16.to_le_bytes());
    response.extend_from_slice(&[0, 0]); // board type and capabilities
    response.extend_from_slice(b"MSP_MOCK\0");
    response.extend_from_slice(b"MSP Mock\0");
    response.extend_from_slice(b"RINZ\0");
    response.extend_from_slice(&[0; 32]); // signature
    response.extend_from_slice(&[0, 0]); // MCU type and configuration state
    response
}

fn write_packet(stream: &mut TcpStream, command: u8, payload: &[u8]) -> io::Result<()> {
    let packet = MspPacket {
        cmd: command,
        direction: MspPacketDirection::FromFlightController,
        data: Cow::Borrowed(payload),
    };
    let mut encoded = vec![0; packet.packet_size_bytes()];
    packet
        .serialize(&mut encoded)
        .expect("buffer was sized from packet_size_bytes");
    stream.write_all(&encoded)
}
