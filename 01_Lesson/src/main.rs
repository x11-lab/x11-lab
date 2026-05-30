// use std::io::{Read, Write};
// use std::net::TcpListener;

// fn main() -> std::io::Result<()> {
//     let listener = TcpListener::bind("0.0.0.0:6000")?;
//     println!("Minimal Rust X11 server listening on display :0 / TCP 6000");

//     for stream in listener.incoming() {
//         let mut stream = stream?;
//         println!("X11 client connected");

//         let mut header = [0u8; 12];
//         stream.read_exact(&mut header)?;

//         let byte_order = header[0] as char;
//         let major = u16::from_le_bytes([header[2], header[3]]);
//         let minor = u16::from_le_bytes([header[4], header[5]]);
//         let auth_name_len = u16::from_le_bytes([header[6], header[7]]) as usize;
//         let auth_data_len = u16::from_le_bytes([header[8], header[9]]) as usize;

//         println!("byte order: {byte_order}");
//         println!("protocol version: {major}.{minor}");
//         println!("auth name length: {auth_name_len}");
//         println!("auth data length: {auth_data_len}");

//         let auth_total = pad4(auth_name_len) + pad4(auth_data_len);
//         let mut auth = vec![0u8; auth_total];
//         stream.read_exact(&mut auth)?;

//         println!("received X11 setup request");

//         // Minimal failure response for now.
//         // This proves the app is receiving real X11 traffic.
//         let reason = b"Rust X11 lab received your connection, but full setup is not implemented yet.";
//         let mut response = Vec::new();
//         response.push(0); // failure
//         response.push(reason.len() as u8);
//         response.extend_from_slice(&11u16.to_le_bytes());
//         response.extend_from_slice(&0u16.to_le_bytes());
//         response.extend_from_slice(reason);

//         stream.write_all(&response)?;
//         println!("sent intentional X11 setup failure");
//     }

//     Ok(())
// }

// fn pad4(value: usize) -> usize {
//     (value + 3) & !3
// }
use std::io::Read;
use std::net::TcpListener;

fn pad4(value: usize) -> usize {
    (value + 3) & !3
}

fn main() -> std::io::Result<()> {
    let listener = TcpListener::bind("0.0.0.0:6000")?;

    println!("Listening on 0.0.0.0:6000");
    println!("Waiting for an X11 setup request...");

    for stream in listener.incoming() {
        match stream {
            Ok(mut stream) => {
                println!("Client connected from {}", stream.peer_addr()?);

                let mut header = [0u8; 12];
                println!("Reading 12-byte header...");
                stream.read_exact(&mut header)?;
                println!("Header read complete");

                println!("{:02X?}", header);

                let byte_order = header[0] as char;
                let major = u16::from_le_bytes([header[2], header[3]]);
                let minor = u16::from_le_bytes([header[4], header[5]]);
                let auth_name_len =
                    u16::from_le_bytes([header[6], header[7]]) as usize;
                let auth_data_len =
                    u16::from_le_bytes([header[8], header[9]]) as usize;

                println!("byte order: {byte_order}");
                println!("protocol version: {major}.{minor}");
                println!("auth name length: {auth_name_len}");
                println!("auth data length: {auth_data_len}");

                let padded_name_len = pad4(auth_name_len);
                let padded_data_len = pad4(auth_data_len);
                let auth_total = padded_name_len + padded_data_len;

                println!("padded auth name length: {padded_name_len}");
                println!("padded auth data length: {padded_data_len}");
                println!("about to read padded auth bytes: {auth_total}");

                let mut auth = vec![0u8; auth_total];
                stream.read_exact(&mut auth)?;

                println!("padded auth bytes read: {auth_total}");
                println!("received X11 setup request");
            }
            Err(err) => {
                eprintln!("Connection error: {err}");
            }
        }
    }

    Ok(())
}