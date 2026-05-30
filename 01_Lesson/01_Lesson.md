# Lesson 01: Accept an X11 Client Connection

In this lesson, we will make the first real connection between an X11 client and
our Rust X11 server.

The client will be `xclock`, a small X11 program that displays a clock. The
server will be our Rust application running on Windows and listening on TCP port
`6000`, which is the standard port for X11 display `:0`.

At the end of this lesson, `xclock` will not show a clock window yet. That is
expected. The goal for Lesson 01 is smaller and more important: prove that a real
X11 client can reach our Rust program, send the X11 setup request, and receive an
intentional failure response.

## What We Are Building

The Rust application for this lesson will:

1. Bind to `0.0.0.0:6000`.
2. Wait for an X11 client to connect.
3. Read the 12-byte X11 setup request header.
4. Parse the fields we care about.
5. Read and discard any authentication bytes.
6. Send an intentional setup failure response.
7. Print enough information to confirm each step worked.

This is the smallest useful X11 server we can build. It does not create windows,
draw pixels, handle events, or manage resources yet. Those come later.

## Why xclock?

`xclock` is a simple graphical clock program that uses X11. It is useful in this
lab because it is small, common, and easy to run from Ubuntu. It does not need a
large desktop environment, and it immediately tries to connect to an X11 server
when it starts.

That makes `xclock` a good test client. If our Rust server is listening in the
right place, running `xclock` should cause the server to print connection and
handshake information.

In this lesson, `xclock` is not important because it draws a clock. It is
important because it behaves like a normal X11 client:

1. It reads the `DISPLAY` environment variable.
2. It opens a connection to the X11 server named by `DISPLAY`.
3. It sends an X11 setup request.
4. It waits for the server to accept, reject, or challenge the connection.

## How the X11 Client Connection Works

X11 uses a client/server model. The graphical program is the client. The display
system is the server.

For this lesson, the flow looks like this:

```text
WSL Ubuntu
    runs xclock
        |
        v
DISPLAY=<windows-host-ip>:0.0
        |
        v
Rust app on Windows
    listens on TCP 6000
    accepts the connection
    reads the X11 setup request
    logs what it received
    sends an intentional setup failure
```

The `DISPLAY` value tells `xclock` where to connect. The `:0.0` part means
display `0`, screen `0`. When X11 uses TCP, display `0` maps to TCP port `6000`.
Display `1` would map to TCP port `6001`.

The first bytes sent by the client are the X11 setup request. They include:

- The byte order the client will use for numeric fields.
- The X11 protocol version requested by the client.
- The length of any authentication protocol name.
- The length of any authentication data.

Our Rust server will read those fields and print them. That is enough to prove
that we are receiving real X11 protocol traffic.

## Prerequisites

You will need:

- Windows 10 build 19041 or newer, or Windows 11.
- WSL installed with Ubuntu.
- Rust installed on Windows.
- A terminal for Windows PowerShell.
- A terminal for Ubuntu in WSL.

This lab assumes the Rust server runs on Windows and the X11 client runs in WSL
Ubuntu. That lets us test the network behavior directly.

## Check and Start WSL Ubuntu
  
Open a PowerShell terminal.

Check your WSL distributions. If you are not using the Ubuntu image your commands may differ:
> This tutorial assumes WSL is already installed and configured; it does not cover the setup process.

```powershell
wsl --list --verbose
```
  
Example output:  
```powershell
PS C:\> wsl --list --verbose
  NAME                      STATE           VERSION
* Ubuntu                    Stopped         2
  podman-machine-default    Running         2
```
  
In this example:  
* `Ubuntu` is installed and configured as the default WSL distribution (indicated by `*`).
* The Ubuntu VM is currently **Stopped**.
* `podman-machine-default` is a separate WSL VM used by Podman and can be ignored for this tutorial.
You should see an Ubuntu distribution in the list.  

### Start the Ubuntu Distribution
  
To explicitly start and enter the Ubuntu WSL instance, run:  

```powershell
wsl --distribution Ubuntu
```
  
You should see a Linux shell prompt similar to:  
  
```bash
user1@DESKTOP-XXXXX:~$
```
At this point, the Ubuntu WSL VM is running and ready for the next steps.

## Install xclock in Ubuntu

Open Ubuntu in WSL and install the X11 sample applications and additional utilities required for testing:

```bash
sudo apt update
sudo apt install -y x11-apps netcat-openbsd
```

Verify that `xclock` exists:

```bash
which xclock

# Example Output: /usr/bin/xclock

xclock -help

# Example Output:
# Usage: xclock [-analog] [-bw <pixels>] [-digital] [-brief]
#        [-utime] [-strftime <fmt-str>]
#        [-fg <color>] [-bg <color>] [-hd <color>]
#        [-hl <color>] [-bd <color>]
#        [-fn <font_name>] [-help] [-padding <pixels>]
#        [-rv] [-update <seconds>] [-display displayname]
#        [-[no]render] [-face <face name>] [-sharp]
#        [-geometry geom] [-twelve] [-twentyfour]
```

## Check Rust on Windows

From PowerShell, verify that Rust and Cargo are installed:

```powershell
rustc --version
cargo --version
```

Then move into the lesson project:

```powershell
cd 01_Lesson
```

This directory contains a normal Cargo binary project:

```text
01_Lesson/
    Cargo.toml
    Cargo.lock
    src/
        main.rs
```

## Step 1: Listen on the X11 Port

The first job of the server is to claim the X11 network address and wait there.
For display `:0`, the X11 TCP port is `6000`. The mapping is simple:

```text
display :0 -> TCP 6000
display :1 -> TCP 6001
display :2 -> TCP 6002
```

That is why this lesson starts with port `6000`. When we later set
`DISPLAY=<windows-host-ip>:0.0` inside WSL, `xclock` will translate display `:0`
into a TCP connection to port `6000` on the Windows host.

Start with this Rust program:

```rust
use std::net::TcpListener;
use std::thread;
use std::time::Duration;

fn main() -> std::io::Result<()> {
    let _listener = TcpListener::bind("0.0.0.0:6000")?;
    println!("Minimal Rust X11 server listening on display :0 / TCP 6000");
    println!("Press Ctrl+C to stop the server");

    loop {
        thread::sleep(Duration::from_secs(60));
    }
}
```

Binding to `0.0.0.0` means the server listens on all network interfaces. That is
convenient for WSL because the client may connect through a virtual network
interface rather than through plain `localhost`. The `_listener` variable must
stay alive. If it is dropped, Rust closes the listening socket and nothing can
connect to port `6000`.

Run the server:

```powershell
cargo run
```

Expected result:

```text
Minimal Rust X11 server listening on display :0 / TCP 6000
Press Ctrl+C to stop the server
```
  
You may receive a notice "Do you want to allow public and private networks to access this app?"  
Click `Allow`  
  
If you see an error that the address is already in use, another program is
already listening on port `6000`. Stop that program before continuing.

Leave the Rust application running. Open your Ubuntu WSL terminal and test that
WSL can reach port `6000` on the Windows host:

```bash
WINDOWS_HOST=$(ip route | awk '/default/ {print $3; exit}')
nc -vz $WINDOWS_HOST 6000
```
Expected result:  
```test
Connection to 172.18.224.1 6000 port [tcp/x11] succeeded!
```

This only proves that the TCP port is reachable. We are not speaking X11 yet,
and the Rust program will not print `X11 client connected` until we add the
accept loop in the next step.

When this checkpoint works, return to the PowerShell terminal running the Rust
application and press `Ctrl+C` to stop it before continuing to Step 2.

## Step 2: Accept a Client

Next, replace the rust in the main.rs file with the following. This keeps the
program running and gives Rust a chance to accept each client connection:

```rust
use std::net::TcpListener;

fn main() -> std::io::Result<()> {
    let listener = TcpListener::bind("0.0.0.0:6000")?;

    println!("Listening on 0.0.0.0:6000");

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                println!(
                    "Client connected from {}",
                    stream.peer_addr()?
                );
            }
            Err(err) => {
                eprintln!("Connection error: {err}");
            }
        }
    }

    Ok(())
}
```

Run the server again:

```powershell
cargo run
```

Leave the Rust application running. Open your Ubuntu WSL terminal and test that
WSL can reach port `6000` on the Windows host:

```bash
WINDOWS_HOST=$(ip route | awk '/default/ {print $3; exit}')
nc -vz $WINDOWS_HOST 6000
```
Expected result from the WSL terminal:  
```text
Connection to 172.18.224.1 6000 port [tcp/x11] succeeded!
```
Expected result from the Windows PowerShell terminal:
```text
# Example Output
Client connected from 172.18.227.201:60842
```

This does not understand X11 yet. It only proves that a client can reach the
server over TCP.

When this checkpoint works, return to the PowerShell terminal running the Rust
application and press `Ctrl+C` to stop it before continuing to Step 3.

## Step 3: Point xclock at the Rust Server

In the WSL terminal make sure the previously assigned environment variable is set:

```bash
echo $WINDOWS_HOST
```

Then point X11 clients at the Rust server:

```bash
export DISPLAY=$WINDOWS_HOST:0.0
```

Run `xclock`:

```bash
xclock
```

At this stage, the Rust program should print:

```text
Client connected from xxx.xxx.xxx.xxx:xxxxx
```
  
If nothing appears in the Rust server output, the client did not reach the Rust process. Check the `DISPLAY` value, confirm the server is running, and allow the program through Windows Firewall if prompted.  

When this checkpoint works, return to the PowerShell terminal running the Rust
application and press `Ctrl+C` to stop it before continuing to Step 4.

## Step 4: Read the X11 Setup Header

After the TCP connection opens, the X11 client sends a 12-byte setup header.
Read it exactly:

```rust
use std::io::Read;

let mut header = [0u8; 12];
stream.read_exact(&mut header)?;
```

For a normal little-endian WSL Ubuntu client, the first byte is usually `l`. That means following numeric values are little-endian. Later lessons can make the parser fully byte-order aware.

Replace `main.rs` with this Step 4 test version:
```rust
use std::io::Read;
use std::net::TcpListener;

fn main() -> std::io::Result<()> {
    let listener = TcpListener::bind("0.0.0.0:6000")?;

    println!("Listening on 0.0.0.0:6000");
    println!("Waiting for an X11 client setup header...");

    for stream in listener.incoming() {
        match stream {
            Ok(mut stream) => {
                println!("Client connected from {}", stream.peer_addr()?);

                let mut header = [0u8; 12];

                match stream.read_exact(&mut header) {
                    Ok(()) => {
                        println!("Received 12-byte X11 setup header:");
                        println!("{:02X?}", header);

                        let byte_order = header[0] as char;
                        println!("Byte order marker: {}", byte_order);

                        match byte_order {
                            'l' => println!("Client is little-endian"),
                            'B' => println!("Client is big-endian"),
                            other => println!("Unexpected byte order marker: {}", other),
                        }
                    }
                    Err(err) => {
                        eprintln!("Failed to read X11 setup header: {err}");
                    }
                }
            }
            Err(err) => {
                eprintln!("Connection error: {err}");
            }
        }
    }

    Ok(())
}
```
Run it from PowerShell:
```powershell
cargo run
```
Then from WSL:
```bash
WINDOWS_HOST=$(ip route | awk '/default/ {print $3; exit}')
export DISPLAY=$WINDOWS_HOST:0.0
xclock
```
Expected Rust output should look similar to:  
```powershell
Listening on 0.0.0.0:6000
Waiting for an X11 client setup header...
Client connected from 172.18.227.201:60842
Received 12-byte X11 setup header:
[6C, 00, 0B, 00, 00, 00, 12, 00, 10, 00, 00, 00]
Byte order marker: l
Client is little-endian
```
`xclock` will probably fail or hang after that. That is okay. The Rust server has only read the X11 setup request; it has not sent a valid X11 setup reply yet.  

When this checkpoint works, return to the PowerShell terminal running the Rust
application and press `Ctrl+C` to stop it before continuing to Step 5.

## Step 5: Parse the X11 Setup Header Fields
  
In Step 4, we proved that an X11 client sends a 12-byte setup header immediately after establishing a TCP connection. We displayed those bytes in hexadecimal form, but we did not yet interpret their meaning.  
  
The X11 protocol defines a fixed layout for these first 12 bytes. Each field tells the server something about the client and how the rest of the setup request should be processed.  
  
For a typical WSL Ubuntu client, the header looks similar to:    

```
[6C, 00, 0B, 00, 00, 00, 12, 00, 10, 00, 00, 00]
```

Those bytes are not random. They contain several important pieces of information:
| Bytes | Meaning                                                |
| ----- | ------------------------------------------------------ |
| 0     | Byte order (`l` for little-endian, `B` for big-endian) |
| 1     | Unused                                                 |
| 2-3   | Major X11 protocol version                             |
| 4-5   | Minor X11 protocol version                             |
| 6-7   | Length of authorization protocol name                  |
| 8-9   | Length of authorization data                           |
| 10-11 | Unused                                                 |
  
The X11 protocol has existed for decades and was designed to support systems with different CPU architectures. The first byte tells the server how multi-byte integers should be interpreted. Most modern Linux systems running on x86-64 use little-endian encoding and therefore send the character `l`.  

The protocol version fields identify which version of the X11 protocol the client expects to use. Modern X11 clients typically request version **11.0**, which is why we expect `xclock` to report protocol version `11.0`.  
  
The authorization fields tell the server how much authentication information follows the setup header. We are not going to process the authentication data yet, but we can determine its size from these fields.  
  
To extract these values from the header, we convert the appropriate bytes into Rust data types:  
  
```rust
let byte_order = header[0] as char;
let major = u16::from_le_bytes([header[2], header[3]]);
let minor = u16::from_le_bytes([header[4], header[5]]);
let auth_name_len = u16::from_le_bytes([header[6], header[7]]) as usize;
let auth_data_len = u16::from_le_bytes([header[8], header[9]]) as usize;

println!("byte order: {byte_order}");
println!("protocol version: {major}.{minor}");
println!("auth name length: {auth_name_len}");
println!("auth data length: {auth_data_len}");
```
When `xclock` connects, you should see protocol version `11.0`.

Replace `main.rs` with this:

```rust
use std::io::Read;
use std::net::TcpListener;

fn main() -> std::io::Result<()> {
    let listener = TcpListener::bind("0.0.0.0:6000")?;

    println!("Listening on 0.0.0.0:6000");
    println!("Waiting for an X11 client setup header...");

    for stream in listener.incoming() {
        match stream {
            Ok(mut stream) => {
                println!("Client connected from {}", stream.peer_addr()?);

                let mut header = [0u8; 12];
                stream.read_exact(&mut header)?;

                println!("Received 12-byte X11 setup header:");
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
            }
            Err(err) => {
                eprintln!("Connection error: {err}");
            }
        }
    }

    Ok(())
}
```
  
Run the server:  

```powershell
cargo run
```
  
Then from WSL:  
  
```bash
export DISPLAY=$WINDOWS_HOST:0.0
xclock
```
Expected output:
```
Listening on 0.0.0.0:6000
Waiting for an X11 client setup header...
Client connected from 172.18.227.201:60842
Received 12-byte X11 setup header:
[6C, 00, 0B, 00, 00, 00, 12, 00, 10, 00, 00, 00]
byte order: l
protocol version: 11.0
auth name length: 18
auth data length: 16
```
At this point we have successfully decoded the fixed-length X11 setup header. The next step is to read the variable-length authorization information that immediately follows it.  

When this checkpoint works, return to the PowerShell terminal running the Rust
application and press `Ctrl+C` to stop it before continuing to Step 6.

## Step 6: Read the Authentication Data
  
In Step 5, we parsed the fixed 12-byte X11 setup header. Two of those fields told us how much authentication data follows the header:  
  
```rust
let auth_name_len = u16::from_le_bytes([header[6], header[7]]) as usize;
let auth_data_len = u16::from_le_bytes([header[8], header[9]]) as usize;
```
The X11 setup request does not stop at the 12-byte header. After the header, the client sends:  
```
authorization protocol name
padding
authorization protocol data
padding
```
The important detail is that X11 pads these fields to 4-byte boundaries. That means a field with a logical length of `18` bytes is actually followed by enough padding to make it occupy `20` bytes on the wire.  
  
To calculate that padded size, add this helper function:  
```rust
fn pad4(value: usize) -> usize {
    (value + 3) & !3
}
```
Then read the full padded authentication area:
```rust
let auth_total = pad4(auth_name_len) + pad4(auth_data_len);
let mut auth = vec![0u8; auth_total];
stream.read_exact(&mut auth)?;

println!("received X11 setup request");
```
  
We are not accepting or validating authentication yet. We are only consuming the correct number of bytes so the stream stays aligned for the next part of the X11 protocol.  
  
Replace `main.rs` with this:  
```rust
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
                stream.read_exact(&mut header)?;

                println!("Received 12-byte X11 setup header:");
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

                let auth_total = pad4(auth_name_len) + pad4(auth_data_len);
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
```
  
Run the server:  

```powershell
cargo run
```
  
Then from WSL:  
  
```bash
export DISPLAY=$WINDOWS_HOST:0.0
xclock
```
Expected output:
```
Listening on 0.0.0.0:6000
Waiting for an X11 setup request...
Client connected from 172.18.227.201:60842
Received 12-byte X11 setup header:
[6C, 00, 0B, 00, 00, 00, 12, 00, 10, 00, 00, 00]
byte order: l
protocol version: 11.0
auth name length: 18
auth data length: 16
padded auth bytes read: 36
received X11 setup request
```
At this point we have read the fixed-length setup header and the padded
authorization data that follows it. The next step is to send a response to the
waiting client.

When this checkpoint works, return to the PowerShell terminal running the Rust
application and press `Ctrl+C` to stop it before continuing to Step 7.

## Step 7: Send an Intentional Failure
  
In Step 6, the Rust server successfully read the complete X11 setup request. The client is now waiting for the server to answer.  
  
For Lesson 01, we are not ready to accept the connection yet. Instead, the server will send a valid X11 setup failure response on purpose. This is still progress: it proves that the Rust application can receive a real X11 setup request and write a protocol-shaped response back to the client.  
  
An X11 setup failure response contains:  
  
| Field               | Meaning                                            |
| ------------------- | -------------------------------------------------- |
| `success`           | `0`, meaning `Failed`                              |
| `reason length`     | Length of the human-readable failure message       |
| `major version`     | X11 major protocol version supported by the server |
| `minor version`     | X11 minor protocol version supported by the server |
| `additional length` | Padded reason length in 4-byte units               |
| `reason`            | The failure message bytes                          |
  
The reason text is padded to a 4-byte boundary, just like the authentication strings we read in Step 6.    
  
The change from Step 6 is that we now write a response after reading the setup request. First, add `Write` to the import list so Rust can call `write_all` on the TCP stream:  
  
```rust
use std::io::{Read, Write};
```
  
Then add this block immediately after:  
  
```rust
println!("received X11 setup request");
```
  
This code builds the setup failure response byte by byte and sends it back to the client:  
  
```rust
let reason = b"Rust X11 lab received your connection, but setup is not implemented yet.\n";
let reason_len = reason.len();
let padded_reason_len = pad4(reason_len);
let additional_length = (padded_reason_len / 4) as u16;

let mut response = Vec::new();
response.push(0); // Failed
response.push(reason_len as u8);
response.extend_from_slice(&11u16.to_le_bytes());
response.extend_from_slice(&0u16.to_le_bytes());
response.extend_from_slice(&additional_length.to_le_bytes());
response.extend_from_slice(reason);
response.resize(8 + padded_reason_len, 0);

stream.write_all(&response)?;
println!("sent intentional X11 setup failure");
```

The first byte, `0`, tells the client the setup failed. The next byte gives the length of the failure reason. The protocol version is still `11.0`. The `additional_length` field tells the client how many padded 4-byte units follow the fixed response header. Finally, `write_all` sends the complete response to `xclock`.  
  
Replace `main.rs` with this:  
  
```rust
use std::io::{Read, Write};
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

                let reason =
                    b"Rust X11 lab received your connection, but setup is not implemented yet.\n";
                let reason_len = reason.len();
                let padded_reason_len = pad4(reason_len);
                let additional_length = (padded_reason_len / 4) as u16;

                let mut response = Vec::new();
                response.push(0); // Failed
                response.push(reason_len as u8);
                response.extend_from_slice(&11u16.to_le_bytes());
                response.extend_from_slice(&0u16.to_le_bytes());
                response.extend_from_slice(&additional_length.to_le_bytes());
                response.extend_from_slice(reason);
                response.resize(8 + padded_reason_len, 0);

                stream.write_all(&response)?;
                println!("sent intentional X11 setup failure");
            }
            Err(err) => {
                eprintln!("Connection error: {err}");
            }
        }
    }

    Ok(())
}
```

Run the server:

```powershell
cargo run
```

Then from WSL:

```bash
export DISPLAY=$WINDOWS_HOST:0.0
xclock
```

Expected Rust output should look similar to:

```text
Listening on 0.0.0.0:6000
Waiting for an X11 setup request...
Client connected from 172.18.227.201:60842
Reading 12-byte header...
Header read complete
[6C, 00, 0B, 00, 00, 00, 12, 00, 10, 00, 00, 00]
byte order: l
protocol version: 11.0
auth name length: 18
auth data length: 16
padded auth name length: 20
padded auth data length: 16
about to read padded auth bytes: 36
padded auth bytes read: 36
received X11 setup request
sent intentional X11 setup failure
```

Expected WSL output:

```text
Rust X11 lab received your connection, but setup is not implemented yet.
Error: Can't open display: 172.18.224.1:0.0
```
  
That error is expected. `xclock` reached our server, but our server deliberately rejected the setup request instead of creating a real display connection.  
  
This intentional failure is a success for the lesson. It proves that:  
  
- `xclock` found our Rust server.
- The Rust server accepted the TCP connection.
- The Rust server read the X11 setup request.
- The Rust server sent bytes back to the X11 client.
  
When this checkpoint works, return to the PowerShell terminal running the Rust application and press `Ctrl+C` to stop it before continuing to Step 8.  
  
## Step 8: Full `main.rs`

The complete Lesson 01 server is:

This final version combines everything added throughout Lesson 01:

- `TcpListener` binds to `0.0.0.0:6000`, the TCP port for X11 display `:0`.
- The server accepts each incoming client and prints the peer address.
- The server reads the fixed 12-byte X11 setup header.
- The server parses byte order, protocol version, and authentication lengths.
- `pad4` calculates how many padded authentication bytes must be consumed.
- The server reads the padded authentication section so the stream stays aligned.
- The server writes an intentional X11 setup failure response back to `xclock`.
  
The key change in the final version is that the server is no longer just observing the client. It now completes the first round trip: client setup request in, server setup failure response out.  
  
```rust
use std::io::{Read, Write};
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

                let reason =
                    b"Rust X11 lab received your connection, but setup is not implemented yet.\n";
                let reason_len = reason.len();
                let padded_reason_len = pad4(reason_len);
                let additional_length = (padded_reason_len / 4) as u16;

                let mut response = Vec::new();
                response.push(0); // Failed
                response.push(reason_len as u8);
                response.extend_from_slice(&11u16.to_le_bytes());
                response.extend_from_slice(&0u16.to_le_bytes());
                response.extend_from_slice(&additional_length.to_le_bytes());
                response.extend_from_slice(reason);
                response.resize(8 + padded_reason_len, 0);

                stream.write_all(&response)?;
                println!("sent intentional X11 setup failure");
            }
            Err(err) => {
                eprintln!("Connection error: {err}");
            }
        }
    }

    Ok(())
}
```
  
This file is the completed Lesson 01 server. It listens for an X11 client, reads the setup request, consumes the padded authorization data, and sends an intentional X11 setup failure response instead of dropping the connection.  

## Step 9: Run the Full Check

Step 9 is the final checkpoint for Lesson 01. There is no new Rust code to add in this step. Instead, we run the complete Lesson 01 server from Step 8 and verify the full path from `xclock` to the Rust process and back to `xclock`.

The full check confirms the complete flow:

```text
xclock starts in WSL
    |
    v
DISPLAY points to <windows-host-ip>:0.0
    |
    v
xclock connects to TCP 6000
    |
    v
Rust reads the X11 setup request
    |
    v
Rust sends an intentional setup failure
    |
    v
xclock prints the failure reason and exits
```
  
Start the Rust server from PowerShell:  
  
```powershell
cargo run
```

Leave that PowerShell window running. In Ubuntu, set `DISPLAY` and run `xclock`:

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
Reading 12-byte header...
Header read complete
[6C, 00, 0B, 00, 00, 00, 12, 00, 10, 00, 00, 00]
byte order: l
protocol version: 11.0
auth name length: 18
auth data length: 16
padded auth name length: 20
padded auth data length: 16
about to read padded auth bytes: 36
padded auth bytes read: 36
received X11 setup request
sent intentional X11 setup failure
```
  
The authentication lengths may differ depending on your environment. That is fine. The important checkpoints are:  
  
- The server prints `Client connected from ...`.  
- The server prints `protocol version: 11.0`.  
- The server prints `received X11 setup request`.  
- The server prints `sent intentional X11 setup failure`.  
  
Expected WSL output:  
  
```text
Rust X11 lab received your connection, but setup is not implemented yet.  
Error: Can't open display: 172.18.224.1:0.0  
```
  
The exact IP address may differ. The important detail is that `xclock` prints the failure reason from our Rust server before reporting that it cannot open the display.
  
That error is expected in Lesson 01. We have not implemented a successful X11 setup response yet. For now, the completed result is that our Rust server can accept the  client, read the setup request, and send a deliberate X11 setup failure response.
  
After the full check works, return to the PowerShell terminal running the Rust application and press `Ctrl+C` to stop it.  
  
## Troubleshooting
  
If `cargo run` fails with `address already in use`, another X11 server or process is already using TCP port `6000`.
  
If `xclock` opens a clock window, it is connecting to another server. Recheck the `DISPLAY` value in the same shell where you run `xclock`.  
  
If `xclock` says it cannot open the display and the Rust server prints nothing, the TCP connection did not reach the Rust process. Check the Windows host IP, Windows Firewall, and whether the Rust server is still running.  
  
If the Rust server prints a connection but then exits with an error, read the last printed field. The next lesson will make the setup parser more robust.  
  
## Lesson 01 Result
  
We now have the first piece of an X11 server: a Rust process that accepts a real X11 client connection and reads the setup request.  
  
In the next lesson, we will replace the intentional failure with a successful X11 setup response. That is the point where clients can begin sending normal X11 requests such as creating windows, asking for atoms, and querying server properties.  

Reference: X.Org X11 protocol documentation, "Connection Setup" and "Protocol
Encoding" sections:
https://x.org/releases/X11R7.7/doc/xproto/x11protocol.html
  
