# Introduction

Welcome to the X11 Lab.

## What Is X11?

X11, also called the X Window System, is a windowing protocol used by many Unix
and Linux desktop environments. It defines how graphical programs create
windows, draw content, receive keyboard and mouse input, and communicate with the
display system.

One detail can be confusing at first: in X11, the graphical application is the
client, and the display system is the server. That means a program like `xclock`
or a terminal emulator is an X11 client. The X11 server is the program that owns
the screen, keyboard, and mouse, and it decides what appears on the display.

This design lets an X11 client run on one machine while the X11 server runs on
another. For example, a Linux program can run inside WSL, in an SSH session, or
on a remote server, then connect back to an X11 server running on your desktop.
The client sends drawing requests over the connection, and the server sends
events such as key presses, mouse movement, window exposure, and close requests
back to the client.

## How X11 Works

An X11 connection starts with the client finding the display address, usually
from the `DISPLAY` environment variable. A display such as `:0` typically maps
to the local X11 server, while a value such as `<host>:0.0` tells the client to
connect to a server on another machine. When X11 uses TCP, display `:0` listens
on port `6000`, display `:1` listens on port `6001`, and so on.

After connecting, the client sends a setup request. This request tells the server
the client's byte order, the X11 protocol version it wants to use, and any
authentication information. The server replies with either a failure, an
authentication challenge, or a successful setup response describing the display.

Once setup succeeds, the connection becomes a stream of protocol messages:

1. The client sends requests such as create a window, draw a line, load a font,
   or ask for a property.
2. The server sends replies for requests that need answers.
3. The server sends errors when a request is invalid.
4. The server sends events when something happens, such as input arriving or a
   window needing to be redrawn.

X11 is built around server-side resources. Windows, cursors, pixmaps, graphics
contexts, fonts, and atoms are all resources identified by numeric IDs. A client
asks the server to create and operate on those resources, and the server tracks
their state.

This lab is about building the server side of that conversation. We will start
with the first bytes of the setup request and gradually teach our Rust program
how to understand and answer more of the protocol.

In this lab, we will use Rust to build a small X11 server step by step. The goal
is not to replace a production X server like Xorg, Xwayland, or XQuartz. The goal
is to understand how an X11 client talks to a display server by implementing the
important pieces ourselves.

X11 is a useful protocol to study because it is old, practical, network-aware,
and still visible in real systems. By writing the server side, we can make those
ideas concrete.

## What We Are Building

We will build a Rust program that listens for X11 clients, accepts their setup
requests, parses protocol messages, and gradually responds with enough behavior
for simple clients to connect and interact with it.

The lab begins with a minimal TCP listener on port `6000`, which corresponds to
display `:0`. From there, each lesson adds one layer:

1. Accept an X11 client connection.
2. Read and inspect the X11 setup request.
3. Send a valid setup response.
4. Decode client requests.
5. Track server-side resources such as windows, graphics contexts, and atoms.
6. Return replies and errors in the format X11 clients expect.
7. Send events back to the client.
8. Build toward enough behavior to support small test clients.

Each step is intentionally small. X11 has a large surface area, so the lab
focuses on the parts needed to understand the protocol and grow a working server
incrementally.

## Why Rust

Rust is a good fit for this lab because an X11 server has to deal with raw bytes,
binary layouts, sockets, shared state, and long-running event loops. Rust lets us
work close to the protocol while still giving us strong tools for correctness:
explicit ownership, checked error handling, structured types, and predictable
memory behavior.

We will avoid hiding the protocol behind a large framework. Instead, we will use
Rust's standard library where possible and introduce helper types only when they
make the server easier to reason about.

## What You Should Know Before Starting

You do not need to be an X11 expert. The lab is designed to introduce the
protocol as we implement it.

You should be comfortable with:

- Basic Rust syntax and running `cargo`.
- Reading and writing bytes.
- TCP client/server concepts.
- Using a terminal on Windows, WSL, Linux, or a remote Linux host.

Some lessons may use a Linux X11 client such as `xclock` to generate real
traffic. The Rust server can run on Windows while the client runs from WSL or
another Linux environment, using the `DISPLAY` environment variable to point the
client at the server.

## Lab Mindset

This is a protocol lab, so the most important habit is to move one verified step
at a time. We will often start by logging bytes, then parsing them, then turning
the parsed data into typed Rust structures, and only then sending a real response.

When something fails, that failure is part of the lesson. X11 clients are strict
about message sizes, byte order, resource IDs, sequence numbers, and event
formats. Small mistakes are expected, and debugging them is how the protocol
becomes understandable.

By the end of the lab, you should have a working mental model of how X11 clients
communicate with a server and a Rust codebase that demonstrates the core pieces
of that conversation.
