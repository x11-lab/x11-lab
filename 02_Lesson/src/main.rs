use std::io::{self, ErrorKind, Read, Write};
use std::net::{TcpListener, TcpStream};

const X11_PORT: &str = "0.0.0.0:6000";
const PROTOCOL_MAJOR: u16 = 11;
const PROTOCOL_MINOR: u16 = 0;
const RESOURCE_ID_BASE: u32 = 0x0020_0000;
const RESOURCE_ID_MASK: u32 = 0x001f_ffff;
const ROOT_WINDOW_ID: u32 = 0x0000_0200;
const DEFAULT_COLORMAP_ID: u32 = 0x0000_0201;
const ROOT_VISUAL_ID: u32 = 0x0000_0021;

#[derive(Clone, Copy)]
enum ByteOrder {
    LittleEndian,
    BigEndian,
}

impl ByteOrder {
    fn from_marker(marker: u8) -> io::Result<Self> {
        match marker {
            b'l' => Ok(Self::LittleEndian),
            b'B' => Ok(Self::BigEndian),
            _ => Err(io::Error::new(
                ErrorKind::InvalidData,
                format!("unexpected X11 byte order marker: 0x{marker:02x}"),
            )),
        }
    }

    fn read_u16(self, bytes: [u8; 2]) -> u16 {
        match self {
            Self::LittleEndian => u16::from_le_bytes(bytes),
            Self::BigEndian => u16::from_be_bytes(bytes),
        }
    }

    fn push_u16(self, out: &mut Vec<u8>, value: u16) {
        match self {
            Self::LittleEndian => out.extend_from_slice(&value.to_le_bytes()),
            Self::BigEndian => out.extend_from_slice(&value.to_be_bytes()),
        }
    }

    fn push_u32(self, out: &mut Vec<u8>, value: u32) {
        match self {
            Self::LittleEndian => out.extend_from_slice(&value.to_le_bytes()),
            Self::BigEndian => out.extend_from_slice(&value.to_be_bytes()),
        }
    }

    fn x11_order_value(self) -> u8 {
        match self {
            Self::LittleEndian => 0,
            Self::BigEndian => 1,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::LittleEndian => "little-endian",
            Self::BigEndian => "big-endian",
        }
    }
}

struct SetupRequest {
    byte_order: ByteOrder,
    major: u16,
    minor: u16,
    auth_name_len: usize,
    auth_data_len: usize,
    auth_total: usize,
}

fn main() -> io::Result<()> {
    let listener = TcpListener::bind(X11_PORT)?;

    println!("Listening on {X11_PORT}");
    println!("Waiting for an X11 setup request...");

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                if let Err(err) = handle_client(stream) {
                    eprintln!("Client error: {err}");
                }
            }
            Err(err) => {
                eprintln!("Connection error: {err}");
            }
        }
    }

    Ok(())
}

fn handle_client(mut stream: TcpStream) -> io::Result<()> {
    println!("Client connected from {}", stream.peer_addr()?);

    let setup = read_setup_request(&mut stream)?;

    println!("byte order: {}", setup.byte_order.label());
    println!("protocol version: {}.{}", setup.major, setup.minor);
    println!("auth name length: {}", setup.auth_name_len);
    println!("auth data length: {}", setup.auth_data_len);
    println!("padded auth bytes read: {}", setup.auth_total);
    println!("received X11 setup request");

    let response = build_setup_success_response(setup.byte_order);
    stream.write_all(&response)?;

    println!(
        "sent successful X11 setup response: {} bytes",
        response.len()
    );
    println!("waiting for normal X11 requests...");

    log_x11_requests(&mut stream, setup.byte_order)
}

fn read_setup_request(stream: &mut TcpStream) -> io::Result<SetupRequest> {
    let mut header = [0u8; 12];
    println!("Reading 12-byte setup header...");
    stream.read_exact(&mut header)?;
    println!("Header read complete");
    println!("{:02X?}", header);

    let byte_order = ByteOrder::from_marker(header[0])?;
    let major = byte_order.read_u16([header[2], header[3]]);
    let minor = byte_order.read_u16([header[4], header[5]]);
    let auth_name_len = byte_order.read_u16([header[6], header[7]]) as usize;
    let auth_data_len = byte_order.read_u16([header[8], header[9]]) as usize;
    let auth_total = pad4(auth_name_len) + pad4(auth_data_len);

    println!("about to read padded auth bytes: {auth_total}");
    let mut auth = vec![0u8; auth_total];
    stream.read_exact(&mut auth)?;

    Ok(SetupRequest {
        byte_order,
        major,
        minor,
        auth_name_len,
        auth_data_len,
        auth_total,
    })
}

fn build_setup_success_response(byte_order: ByteOrder) -> Vec<u8> {
    let vendor = b"x11-lab";
    let mut body = Vec::new();

    byte_order.push_u32(&mut body, 2); // release-number
    byte_order.push_u32(&mut body, RESOURCE_ID_BASE);
    byte_order.push_u32(&mut body, RESOURCE_ID_MASK);
    byte_order.push_u32(&mut body, 0); // motion-buffer-size
    byte_order.push_u16(&mut body, vendor.len() as u16);
    byte_order.push_u16(&mut body, u16::MAX); // maximum-request-length
    body.push(1); // number of screens
    body.push(2); // number of pixmap formats
    body.push(byte_order.x11_order_value()); // image-byte-order
    body.push(byte_order.x11_order_value()); // bitmap-format-bit-order
    body.push(32); // bitmap-format-scanline-unit
    body.push(32); // bitmap-format-scanline-pad
    body.push(8); // min-keycode
    body.push(255); // max-keycode
    body.extend_from_slice(&[0; 4]);

    body.extend_from_slice(vendor);
    pad_to_4(&mut body);

    push_pixmap_format(&mut body, 1, 1, 32);
    push_pixmap_format(&mut body, 24, 32, 32);
    push_screen(&mut body, byte_order);

    assert_eq!(body.len() % 4, 0);

    let additional_length = (body.len() / 4) as u16;
    let mut response = Vec::new();
    response.push(1); // Success
    response.push(0); // unused
    byte_order.push_u16(&mut response, PROTOCOL_MAJOR);
    byte_order.push_u16(&mut response, PROTOCOL_MINOR);
    byte_order.push_u16(&mut response, additional_length);
    response.extend_from_slice(&body);

    response
}

fn push_pixmap_format(out: &mut Vec<u8>, depth: u8, bits_per_pixel: u8, scanline_pad: u8) {
    out.push(depth);
    out.push(bits_per_pixel);
    out.push(scanline_pad);
    out.extend_from_slice(&[0; 5]);
}

fn push_screen(out: &mut Vec<u8>, byte_order: ByteOrder) {
    byte_order.push_u32(out, ROOT_WINDOW_ID);
    byte_order.push_u32(out, DEFAULT_COLORMAP_ID);
    byte_order.push_u32(out, 0x00ff_ffff); // white-pixel
    byte_order.push_u32(out, 0x0000_0000); // black-pixel
    byte_order.push_u32(out, 0); // current-input-masks
    byte_order.push_u16(out, 800); // width-in-pixels
    byte_order.push_u16(out, 600); // height-in-pixels
    byte_order.push_u16(out, 211); // width-in-millimeters
    byte_order.push_u16(out, 158); // height-in-millimeters
    byte_order.push_u16(out, 1); // min-installed-maps
    byte_order.push_u16(out, 1); // max-installed-maps
    byte_order.push_u32(out, ROOT_VISUAL_ID);
    out.push(0); // backing-stores: Never
    out.push(0); // save-unders: false
    out.push(24); // root-depth
    out.push(2); // number of allowed depths

    push_depth_without_visuals(out, byte_order, 1);
    push_depth_with_true_color_visual(out, byte_order);
}

fn push_depth_without_visuals(out: &mut Vec<u8>, byte_order: ByteOrder, depth: u8) {
    out.push(depth);
    out.push(0); // unused
    byte_order.push_u16(out, 0); // number of visuals
    byte_order.push_u32(out, 0); // unused
}

fn push_depth_with_true_color_visual(out: &mut Vec<u8>, byte_order: ByteOrder) {
    out.push(24);
    out.push(0); // unused
    byte_order.push_u16(out, 1); // number of visuals
    byte_order.push_u32(out, 0); // unused

    byte_order.push_u32(out, ROOT_VISUAL_ID);
    out.push(4); // class: TrueColor
    out.push(8); // bits-per-rgb-value
    byte_order.push_u16(out, 256); // colormap-entries
    byte_order.push_u32(out, 0x00ff_0000); // red-mask
    byte_order.push_u32(out, 0x0000_ff00); // green-mask
    byte_order.push_u32(out, 0x0000_00ff); // blue-mask
    byte_order.push_u32(out, 0); // unused
}

fn log_x11_requests(stream: &mut TcpStream, byte_order: ByteOrder) -> io::Result<()> {
    let mut sequence = 1u16;

    loop {
        let mut header = [0u8; 4];
        match stream.read_exact(&mut header) {
            Ok(()) => {}
            Err(err)
                if matches!(
                    err.kind(),
                    ErrorKind::UnexpectedEof
                        | ErrorKind::ConnectionReset
                        | ErrorKind::ConnectionAborted
                ) =>
            {
                println!("client disconnected");
                return Ok(());
            }
            Err(err) => return Err(err),
        }

        let opcode = header[0];
        let data = header[1];
        let length_units = byte_order.read_u16([header[2], header[3]]);

        if length_units == 0 {
            println!(
                "request #{sequence}: opcode {opcode} uses BigRequests length encoding, which Lesson 02 does not implement"
            );
            return Ok(());
        }

        let total_bytes = length_units as usize * 4;
        if total_bytes < 4 {
            println!("request #{sequence}: opcode {opcode} has invalid length {length_units}");
            return Ok(());
        }

        let payload_bytes = total_bytes - 4;
        let mut payload = vec![0u8; payload_bytes];
        stream.read_exact(&mut payload)?;

        println!(
            "request #{sequence}: opcode {opcode} ({}) data {data} length {length_units} ({total_bytes} bytes)",
            request_name(opcode)
        );

        sequence = sequence.wrapping_add(1);
    }
}

fn request_name(opcode: u8) -> &'static str {
    match opcode {
        1 => "CreateWindow",
        2 => "ChangeWindowAttributes",
        3 => "GetWindowAttributes",
        4 => "DestroyWindow",
        8 => "MapWindow",
        12 => "ConfigureWindow",
        16 => "InternAtom",
        18 => "ChangeProperty",
        20 => "GetProperty",
        38 => "QueryPointer",
        43 => "GetInputFocus",
        45 => "OpenFont",
        47 => "QueryFont",
        48 => "QueryTextExtents",
        53 => "CreatePixmap",
        55 => "CreateGC",
        56 => "ChangeGC",
        60 => "FreeGC",
        61 => "ClearArea",
        62 => "CopyArea",
        65 => "PolyLine",
        70 => "PolyFillRectangle",
        72 => "PutImage",
        74 => "PolyText8",
        76 => "ImageText8",
        78 => "CreateColormap",
        84 => "AllocColor",
        98 => "QueryExtension",
        99 => "ListExtensions",
        100 => "ChangeKeyboardMapping",
        101 => "GetKeyboardMapping",
        102 => "ChangeKeyboardControl",
        103 => "GetKeyboardControl",
        104 => "Bell",
        105 => "ChangePointerControl",
        106 => "GetPointerControl",
        107 => "SetScreenSaver",
        108 => "GetScreenSaver",
        116 => "SetPointerMapping",
        117 => "GetPointerMapping",
        118 => "SetModifierMapping",
        119 => "GetModifierMapping",
        127 => "NoOperation",
        _ => "unknown",
    }
}

fn pad4(value: usize) -> usize {
    (value + 3) & !3
}

fn pad_to_4(out: &mut Vec<u8>) {
    while out.len() % 4 != 0 {
        out.push(0);
    }
}
