# Lesson 02: Send a Successful X11 Setup Response
  
In Lesson 01, our Rust server accepted a real X11 client connection, read the setup request, and sent an intentional setup failure. That proved `xclock` could reach our server and that we understood the first bytes of the protocol.  
  
In this lesson, we will replace that intentional failure with a minimal successful X11 setup response.  
  
At the end of this lesson, `xclock` still will not show a clock window. That is expected. The new milestone is that the setup handshake succeeds, so `xclock` starts sending normal X11 requests such as `QueryExtension`, `InternAtom`, `CreateGC`, or `CreateWindow`. We will log those requests instead of handling them.  
  
## What We Are Building
  
The Rust application for this lesson will:  
  
1. Accept an X11 client connection on TCP port `6000`.
2. Read the client's X11 setup request.
3. Respect the client's byte order when parsing and writing protocol fields.
4. Build a minimal successful setup response.
5. Describe one root screen, one root visual, and the pixmap formats we support.
6. Send the setup success response to `xclock`.
7. Read and log normal X11 request headers after setup succeeds.
  
This is the first point where our server behaves like a real X11 peer. The server still does not implement windows, drawing, fonts, atoms, replies, errors, or events. Those come in later lessons.  
  
## Why Setup Success Is Larger Than Setup Failure
  
The setup failure from Lesson 01 was small. It only needed to tell the client that setup failed and include a reason string.  
  
A setup success response is larger because it tells the client what kind of X11 server it connected to. The client needs this information before it can create windows or allocate resources.  
  
The success response includes:  
  
| Field             | Purpose                                                      |
| ----------------- | ------------------------------------------------------------ |
| Protocol version  | Confirms the server speaks X11 version `11.0`                |
| Resource ID range | Tells the client which numeric IDs it may allocate           |
| Vendor string     | Identifies this server implementation                        |
| Pixmap formats    | Describes supported pixmap depths and pixel sizes            |
| Screen list       | Describes the root screen, dimensions, colormap, and visuals |
| Root visual       | Tells clients how colors are represented for the root window |
  
For Lesson 02, we will provide one simple screen:  
  
- Size: `800x600`
- Root depth: `24`
- Visual class: `TrueColor`
- RGB masks: `0x00ff0000`, `0x0000ff00`, `0x000000ff`
- Vendor: `x11-lab`
  
These values are enough to teach the shape of the setup response and move the client past the setup phase.  
  
## Step 1: Copy the Lesson 01 Server
  
Create a new lesson project:  
  
```text
02_Lesson/
    Cargo.toml
    Cargo.lock
    src/
        main.rs
```
  
Lesson 02 starts with the final server from Lesson 01. That server already knows how to:  
  
- Listen on `0.0.0.0:6000`.
- Accept an X11 client.
- Read the 12-byte setup header.
- Read the padded authentication data.
- Send an intentional setup failure response.
  
Run the copied server once before changing it:  
  
```powershell
cd 02_Lesson
cargo run
```
  
From WSL:  
  
```bash
WINDOWS_HOST=$(ip route | awk '/default/ {print $3; exit}')
export DISPLAY=$WINDOWS_HOST:0.0
xclock
```
  
Expected result:  
  
```text
Rust X11 lab received your connection, but setup is not implemented yet.
Error: Can't open display: 172.18.224.1:0.0
```
  
That confirms the Lesson 01 baseline still works.  
  
When this checkpoint works, return to the PowerShell terminal running the Rust application and press `Ctrl+C` before continuing to Step 2.  
  
## Step 2: Add Byte Order Helpers
  
In Lesson 01, we parsed the setup request as little-endian because WSL on x86-64 normally sends the byte order marker `l`. For a real X11 server response, we should write multi-byte values using the byte order requested by the client.  
  
Add this enum and helper implementation:  
  
```rust
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
}
```
  
The important change is that we no longer call `u16::from_le_bytes` everywhere. Instead, we ask the setup request which byte order the client selected and use that order consistently.  
  
When this checkpoint builds, continue to Step 3.  
  
## Step 3: Parse Setup Into a Struct
  
Now collect the parsed setup data into a `SetupRequest` struct:  
  
```rust
struct SetupRequest {
    byte_order: ByteOrder,
    major: u16,
    minor: u16,
    auth_name_len: usize,
    auth_data_len: usize,
    auth_total: usize,
}
```
  
Then move setup parsing into a function:  
  
```rust
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
```
  
This does not change behavior yet. It makes the next steps easier because the server can pass one setup value into the response builder.  
  
When this checkpoint builds, continue to Step 4.  
  
## Step 4: Build the Successful Setup Header
  
The first 8 bytes of a successful setup response say:  
  
| Bytes | Meaning                                            |
| ----- | -------------------------------------------------- |
| `0`   | Status: `1` means success                          |
| `1`   | Unused                                             |
| `2-3` | Protocol major version                             |
| `4-5` | Protocol minor version                             |
| `6-7` | Length of the remaining setup data in 4-byte units |
  
Start the response builder with this shape:  
  
```rust
fn build_setup_success_response(byte_order: ByteOrder) -> Vec<u8> {
    let mut body = Vec::new();

    // Later steps fill body with server, pixmap, and screen information.

    let additional_length = (body.len() / 4) as u16;
    let mut response = Vec::new();
    response.push(1); // Success
    response.push(0); // unused
    byte_order.push_u16(&mut response, 11);
    byte_order.push_u16(&mut response, 0);
    byte_order.push_u16(&mut response, additional_length);
    response.extend_from_slice(&body);

    response
}
```
  
Do not run this version yet. An empty success body is not useful to an X11 client. We need to add server information, pixmap formats, and screen information first.  
  
## Step 5: Add Server Information  
  
The setup body starts with fixed server information. This tells the client which resource IDs it may create, how large normal requests can be, and what server it connected to.  
  
Add constants near the top of `main.rs`:  
  
```rust
const PROTOCOL_MAJOR: u16 = 11;
const PROTOCOL_MINOR: u16 = 0;
const RESOURCE_ID_BASE: u32 = 0x0020_0000;
const RESOURCE_ID_MASK: u32 = 0x001f_ffff;
```
  
Then add the server information fields to the response body:  
  
```rust
let vendor = b"x11-lab";

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
```
  
The resource ID base and mask are important. They tell the client which numeric resource IDs are safe for it to allocate later. In future lessons, when `xclock` creates windows or graphics contexts, those IDs should fall inside this range.  
  
When this checkpoint builds, continue to Step 6.  
  
## Step 6: Add Pixmap Formats
  
After the vendor string, the server lists supported pixmap formats. A format says which depth is supported, how many bits each pixel uses, and how scanlines are padded.
  
Add this helper:  
  
```rust
fn push_pixmap_format(out: &mut Vec<u8>, depth: u8, bits_per_pixel: u8, scanline_pad: u8) {
    out.push(depth);
    out.push(bits_per_pixel);
    out.push(scanline_pad);
    out.extend_from_slice(&[0; 5]);
}
```
  
Then add two formats:  
  
```rust
push_pixmap_format(&mut body, 1, 1, 32);
push_pixmap_format(&mut body, 24, 32, 32);
```
  
Depth `1` is useful for bitmap-style pixmaps. Depth `24` with `32` bits per pixel is a common true-color format on modern systems.  
  
When this checkpoint builds, continue to Step 7.  
  
## Step 7: Add One Screen and One Visual  
  
The screen section describes what the client sees as the display. It includes the root window ID, a default colormap, screen size, root depth, and supported visuals.  
  
Add these constants:  
  
```rust
const ROOT_WINDOW_ID: u32 = 0x0000_0200;
const DEFAULT_COLORMAP_ID: u32 = 0x0000_0201;
const ROOT_VISUAL_ID: u32 = 0x0000_0021;
```
  
Then add the screen helper:  
  
```rust
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
```
  
The root visual is the color model for the root window. In this lesson we use a simple `TrueColor` visual with 8 bits per RGB channel:  
  
```rust
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
```
  
Then call `push_screen` after the pixmap formats:  
  
```rust
push_screen(&mut body, byte_order);
```
  
When this checkpoint builds, continue to Step 8.  
  
## Step 8: Send Success and Log Requests  
   
Now replace the Lesson 01 failure response with the success response:  
  
```rust
let response = build_setup_success_response(setup.byte_order);
stream.write_all(&response)?;

println!(
    "sent successful X11 setup response: {} bytes",
    response.len()
);
println!("waiting for normal X11 requests...");
```
  
After setup succeeds, `xclock` will start sending normal X11 requests. We are not ready to implement those requests yet, but we can read and log their request headers:  
  
```rust
fn log_x11_requests(stream: &mut TcpStream, byte_order: ByteOrder) -> io::Result<()> {
    let mut sequence = 1u16;

    loop {
        let mut header = [0u8; 4];
        stream.read_exact(&mut header)?;

        let opcode = header[0];
        let data = header[1];
        let length_units = byte_order.read_u16([header[2], header[3]]);
        let total_bytes = length_units as usize * 4;

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
```
  
This is the new behavior for Lesson 02. A successful setup response is not the end of the protocol. It is the point where the normal X11 request stream begins.  
  
## Step 9: Full `main.rs`
  
The complete Lesson 02 server is:  
  
```rust
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
            println!(
                "request #{sequence}: opcode {opcode} has invalid length {length_units}"
            );
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
```
  
## Step 10: Run the Full Check  
  
Start the Lesson 02 server from Pow erShell:  
  
```powershell
cd 02_Lesson
cargo run
```

In Ubuntu, point `xclock` at the Rust server:

```bash
WINDOWS_HOST=$(ip route | awk '/default/ {print $3; exit}')
export DISPLAY=$WINDOWS_HOST:0.0
xclock
```

Expected Rust output should look similar to:

```text
Listening on 0.0.0.0:6000
Waiting for an X11 setup request...
Client connected from 172.18.227.201:60842
Reading 12-byte setup header...
Header read complete
[6C, 00, 0B, 00, 00, 00, 12, 00, 10, 00, 00, 00]
about to read padded auth bytes: 36
byte order: little-endian
protocol version: 11.0
auth name length: 18
auth data length: 16
padded auth bytes read: 36
received X11 setup request
sent successful X11 setup response: 144 bytes
waiting for normal X11 requests...
request #1: opcode 98 (QueryExtension) data 0 length 5 (20 bytes)
```
  
The first request may differ depending on your environment. The important result is that the server prints `sent successful X11 setup response` and then begins printing normal X11 requests. That means setup succeeded.  
  
Expected WSL behavior:  
  
- `xclock` should not print the Lesson 01 setup failure reason.  
- `xclock` may hang because the Rust server does not answer normal requests yet.  
- No clock window should appear yet.  
  
When this checkpoint works, return to the PowerShell terminal running the Rust application and press `Ctrl+C` to stop it.  
  
## Troubleshooting  
  
If `xclock` still prints the Lesson 01 failure reason, you are probably running the Lesson 01 server or an old build. Stop the server, move into `02_Lesson`, and run `cargo run` again.  
  
If `xclock` says it cannot open the display and the Rust server prints nothing, check `DISPLAY`, the Windows host IP, and Windows Firewall.  
  
If the Rust server prints `sent successful X11 setup response` but no request headers, the client may be waiting or may have disconnected. Try running `xclock` again from the same WSL shell.  
  
If `xclock` hangs, that is expected. The next lesson will start handling the normal requests that clients send after setup succeeds.  
  
## Lesson 02 Result
  
We now have a Rust program that can complete the X11 setup handshake successfully. The client gets real server information: a resource ID range, pixmap formats, a root screen, and a true-color visual.  
  
The next step is to implement the first normal X11 request handlers. That is where the server begins to answer questions such as which extensions exist, which atoms exist, and how client-created resources should be tracked.  
  
Reference: X.Org X11 protocol documentation, "Connection Setup" and "Protocol  
Encoding" sections:  
https://x.org/releases/X11R7.7/doc/xproto/x11protocol.html  
  