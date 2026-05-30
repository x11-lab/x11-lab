use std::collections::HashMap;
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

const ERROR_REQUEST: u8 = 1;
const ERROR_ATOM: u8 = 5;
const ERROR_LENGTH: u8 = 16;

const OPCODE_INTERN_ATOM: u8 = 16;
const OPCODE_GET_ATOM_NAME: u8 = 17;
const OPCODE_GET_INPUT_FOCUS: u8 = 43;
const OPCODE_QUERY_EXTENSION: u8 = 98;
const OPCODE_LIST_EXTENSIONS: u8 = 99;
const OPCODE_NO_OPERATION: u8 = 127;

const FIRST_DYNAMIC_ATOM: u32 = 68;
const POINTER_ROOT: u32 = 1;

const PREDEFINED_ATOMS: &[(u32, &str)] = &[
    (1, "PRIMARY"),
    (2, "SECONDARY"),
    (3, "ARC"),
    (4, "ATOM"),
    (5, "BITMAP"),
    (6, "CARDINAL"),
    (7, "COLORMAP"),
    (8, "CURSOR"),
    (9, "CUT_BUFFER0"),
    (10, "CUT_BUFFER1"),
    (11, "CUT_BUFFER2"),
    (12, "CUT_BUFFER3"),
    (13, "CUT_BUFFER4"),
    (14, "CUT_BUFFER5"),
    (15, "CUT_BUFFER6"),
    (16, "CUT_BUFFER7"),
    (17, "DRAWABLE"),
    (18, "FONT"),
    (19, "INTEGER"),
    (20, "PIXMAP"),
    (21, "POINT"),
    (22, "RECTANGLE"),
    (23, "RESOURCE_MANAGER"),
    (24, "RGB_COLOR_MAP"),
    (25, "RGB_BEST_MAP"),
    (26, "RGB_BLUE_MAP"),
    (27, "RGB_DEFAULT_MAP"),
    (28, "RGB_GRAY_MAP"),
    (29, "RGB_GREEN_MAP"),
    (30, "RGB_RED_MAP"),
    (31, "STRING"),
    (32, "VISUALID"),
    (33, "WINDOW"),
    (34, "WM_COMMAND"),
    (35, "WM_HINTS"),
    (36, "WM_CLIENT_MACHINE"),
    (37, "WM_ICON_NAME"),
    (38, "WM_ICON_SIZE"),
    (39, "WM_NAME"),
    (40, "WM_NORMAL_HINTS"),
    (41, "WM_SIZE_HINTS"),
    (42, "WM_ZOOM_HINTS"),
    (43, "MIN_SPACE"),
    (44, "NORM_SPACE"),
    (45, "MAX_SPACE"),
    (46, "END_SPACE"),
    (47, "SUPERSCRIPT_X"),
    (48, "SUPERSCRIPT_Y"),
    (49, "SUBSCRIPT_X"),
    (50, "SUBSCRIPT_Y"),
    (51, "UNDERLINE_POSITION"),
    (52, "UNDERLINE_THICKNESS"),
    (53, "STRIKEOUT_ASCENT"),
    (54, "STRIKEOUT_DESCENT"),
    (55, "ITALIC_ANGLE"),
    (56, "X_HEIGHT"),
    (57, "QUAD_WIDTH"),
    (58, "WEIGHT"),
    (59, "POINT_SIZE"),
    (60, "RESOLUTION"),
    (61, "COPYRIGHT"),
    (62, "NOTICE"),
    (63, "FONT_NAME"),
    (64, "FAMILY_NAME"),
    (65, "FULL_NAME"),
    (66, "CAP_HEIGHT"),
    (67, "WM_CLASS"),
];

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

    fn read_u32(self, bytes: [u8; 4]) -> u32 {
        match self {
            Self::LittleEndian => u32::from_le_bytes(bytes),
            Self::BigEndian => u32::from_be_bytes(bytes),
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

struct Request {
    sequence: u16,
    opcode: u8,
    data: u8,
    length_units: u16,
    payload: Vec<u8>,
}

impl Request {
    fn total_bytes(&self) -> usize {
        usize::from(self.length_units) * 4
    }
}

struct AtomTable {
    name_to_id: HashMap<Vec<u8>, u32>,
    id_to_name: HashMap<u32, Vec<u8>>,
    next_dynamic_atom: u32,
}

impl AtomTable {
    fn new() -> Self {
        let mut atoms = Self {
            name_to_id: HashMap::new(),
            id_to_name: HashMap::new(),
            next_dynamic_atom: FIRST_DYNAMIC_ATOM,
        };

        for (id, name) in PREDEFINED_ATOMS {
            atoms.insert_fixed(*id, name.as_bytes());
        }

        atoms
    }

    fn insert_fixed(&mut self, id: u32, name: &[u8]) {
        let name = name.to_vec();
        self.name_to_id.insert(name.clone(), id);
        self.id_to_name.insert(id, name);
    }

    fn intern(&mut self, name: &[u8], only_if_exists: bool) -> u32 {
        if let Some(id) = self.name_to_id.get(name) {
            return *id;
        }

        if only_if_exists {
            return 0;
        }

        let id = self.next_dynamic_atom;
        self.next_dynamic_atom += 1;

        let name = name.to_vec();
        self.name_to_id.insert(name.clone(), id);
        self.id_to_name.insert(id, name);

        id
    }

    fn name(&self, id: u32) -> Option<&[u8]> {
        self.id_to_name.get(&id).map(Vec::as_slice)
    }
}

fn main() -> io::Result<()> {
    let listener = TcpListener::bind(X11_PORT)?;
    let mut atoms = AtomTable::new();

    println!("Listening on {X11_PORT}");
    println!("Waiting for an X11 setup request...");

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                if let Err(err) = handle_client(stream, &mut atoms) {
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

fn handle_client(mut stream: TcpStream, atoms: &mut AtomTable) -> io::Result<()> {
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

    handle_x11_requests(&mut stream, setup.byte_order, atoms)
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

fn handle_x11_requests(
    stream: &mut TcpStream,
    byte_order: ByteOrder,
    atoms: &mut AtomTable,
) -> io::Result<()> {
    let mut sequence = 1u16;

    loop {
        let Some(request) = read_x11_request(stream, byte_order, sequence)? else {
            return Ok(());
        };

        println!(
            "request #{}: opcode {} ({}) data {} length {} ({} bytes)",
            request.sequence,
            request.opcode,
            request_name(request.opcode),
            request.data,
            request.length_units,
            request.total_bytes()
        );

        match request.opcode {
            OPCODE_QUERY_EXTENSION => handle_query_extension(stream, byte_order, &request)?,
            OPCODE_LIST_EXTENSIONS => handle_list_extensions(stream, byte_order, &request)?,
            OPCODE_INTERN_ATOM => handle_intern_atom(stream, byte_order, &request, atoms)?,
            OPCODE_GET_ATOM_NAME => handle_get_atom_name(stream, byte_order, &request, &*atoms)?,
            OPCODE_GET_INPUT_FOCUS => handle_get_input_focus(stream, byte_order, &request)?,
            OPCODE_NO_OPERATION => println!("  NoOperation: no reply needed"),
            _ => {
                println!("  unsupported request: sending Request error");
                send_error(stream, byte_order, &request, ERROR_REQUEST, 0)?;
            }
        }

        sequence = sequence.wrapping_add(1);
    }
}

fn read_x11_request(
    stream: &mut TcpStream,
    byte_order: ByteOrder,
    sequence: u16,
) -> io::Result<Option<Request>> {
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
            return Ok(None);
        }
        Err(err) => return Err(err),
    }

    let opcode = header[0];
    let data = header[1];
    let length_units = byte_order.read_u16([header[2], header[3]]);

    if length_units == 0 {
        println!(
            "request #{sequence}: opcode {opcode} uses BigRequests length encoding, which Lesson 03 does not implement"
        );
        return Ok(None);
    }

    let total_bytes = usize::from(length_units) * 4;
    if total_bytes < 4 {
        println!("request #{sequence}: opcode {opcode} has invalid length {length_units}");
        return Ok(None);
    }

    let mut payload = vec![0u8; total_bytes - 4];
    stream.read_exact(&mut payload)?;

    Ok(Some(Request {
        sequence,
        opcode,
        data,
        length_units,
        payload,
    }))
}

fn handle_query_extension(
    stream: &mut TcpStream,
    byte_order: ByteOrder,
    request: &Request,
) -> io::Result<()> {
    let Some(name_len) = read_u16_at(byte_order, &request.payload, 0) else {
        return send_length_error(stream, byte_order, request);
    };

    let name_len = usize::from(name_len);
    if request.payload.len() < 4 + name_len {
        return send_length_error(stream, byte_order, request);
    }

    let name = &request.payload[4..4 + name_len];
    println!(
        "  QueryExtension \"{}\": not present",
        String::from_utf8_lossy(name)
    );

    let mut reply = Vec::with_capacity(32);
    push_reply_header(&mut reply, byte_order, request.sequence, 0, 0);
    reply.push(0); // present: false
    reply.push(0); // major opcode
    reply.push(0); // first event
    reply.push(0); // first error
    reply.extend_from_slice(&[0; 20]);
    debug_assert_eq!(reply.len(), 32);

    stream.write_all(&reply)
}

fn handle_list_extensions(
    stream: &mut TcpStream,
    byte_order: ByteOrder,
    request: &Request,
) -> io::Result<()> {
    println!("  ListExtensions: returning an empty extension list");

    let mut reply = Vec::with_capacity(32);
    push_reply_header(&mut reply, byte_order, request.sequence, 0, 0);
    reply.extend_from_slice(&[0; 24]);
    debug_assert_eq!(reply.len(), 32);

    stream.write_all(&reply)
}

fn handle_intern_atom(
    stream: &mut TcpStream,
    byte_order: ByteOrder,
    request: &Request,
    atoms: &mut AtomTable,
) -> io::Result<()> {
    let Some(name_len) = read_u16_at(byte_order, &request.payload, 0) else {
        return send_length_error(stream, byte_order, request);
    };

    let name_len = usize::from(name_len);
    if request.payload.len() < 4 + name_len {
        return send_length_error(stream, byte_order, request);
    }

    let only_if_exists = request.data != 0;
    let name = &request.payload[4..4 + name_len];
    let atom = atoms.intern(name, only_if_exists);

    println!(
        "  InternAtom \"{}\" only-if-exists={} -> {atom}",
        String::from_utf8_lossy(name),
        only_if_exists
    );

    let mut reply = Vec::with_capacity(32);
    push_reply_header(&mut reply, byte_order, request.sequence, 0, 0);
    byte_order.push_u32(&mut reply, atom);
    reply.extend_from_slice(&[0; 20]);
    debug_assert_eq!(reply.len(), 32);

    stream.write_all(&reply)
}

fn handle_get_atom_name(
    stream: &mut TcpStream,
    byte_order: ByteOrder,
    request: &Request,
    atoms: &AtomTable,
) -> io::Result<()> {
    let Some(atom) = read_u32_at(byte_order, &request.payload, 0) else {
        return send_length_error(stream, byte_order, request);
    };

    let Some(name) = atoms.name(atom) else {
        println!("  GetAtomName {atom}: unknown atom");
        return send_error(stream, byte_order, request, ERROR_ATOM, atom);
    };

    println!(
        "  GetAtomName {atom} -> \"{}\"",
        String::from_utf8_lossy(name)
    );

    let padded_name_len = pad4(name.len());
    let additional_length = (padded_name_len / 4) as u32;

    let mut reply = Vec::with_capacity(32 + padded_name_len);
    push_reply_header(
        &mut reply,
        byte_order,
        request.sequence,
        0,
        additional_length,
    );
    byte_order.push_u16(&mut reply, name.len() as u16);
    reply.extend_from_slice(&[0; 22]);
    reply.extend_from_slice(name);
    reply.resize(32 + padded_name_len, 0);

    stream.write_all(&reply)
}

fn handle_get_input_focus(
    stream: &mut TcpStream,
    byte_order: ByteOrder,
    request: &Request,
) -> io::Result<()> {
    println!("  GetInputFocus: returning PointerRoot");

    let mut reply = Vec::with_capacity(32);
    push_reply_header(&mut reply, byte_order, request.sequence, 1, 0);
    byte_order.push_u32(&mut reply, POINTER_ROOT);
    reply.extend_from_slice(&[0; 20]);
    debug_assert_eq!(reply.len(), 32);

    stream.write_all(&reply)
}

fn send_length_error(
    stream: &mut TcpStream,
    byte_order: ByteOrder,
    request: &Request,
) -> io::Result<()> {
    println!("  malformed request length: sending Length error");
    send_error(
        stream,
        byte_order,
        request,
        ERROR_LENGTH,
        u32::from(request.length_units),
    )
}

fn send_error(
    stream: &mut TcpStream,
    byte_order: ByteOrder,
    request: &Request,
    error_code: u8,
    bad_value: u32,
) -> io::Result<()> {
    let mut error = Vec::with_capacity(32);
    error.push(0); // Error
    error.push(error_code);
    byte_order.push_u16(&mut error, request.sequence);
    byte_order.push_u32(&mut error, bad_value);
    byte_order.push_u16(&mut error, 0); // minor opcode
    error.push(request.opcode);
    error.extend_from_slice(&[0; 21]);
    debug_assert_eq!(error.len(), 32);

    stream.write_all(&error)
}

fn push_reply_header(
    out: &mut Vec<u8>,
    byte_order: ByteOrder,
    sequence: u16,
    data: u8,
    additional_length: u32,
) {
    out.push(1); // Reply
    out.push(data);
    byte_order.push_u16(out, sequence);
    byte_order.push_u32(out, additional_length);
}

fn read_u16_at(byte_order: ByteOrder, bytes: &[u8], offset: usize) -> Option<u16> {
    let bytes = bytes.get(offset..offset + 2)?;
    Some(byte_order.read_u16([bytes[0], bytes[1]]))
}

fn read_u32_at(byte_order: ByteOrder, bytes: &[u8], offset: usize) -> Option<u32> {
    let bytes = bytes.get(offset..offset + 4)?;
    Some(byte_order.read_u32([bytes[0], bytes[1], bytes[2], bytes[3]]))
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
        17 => "GetAtomName",
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
