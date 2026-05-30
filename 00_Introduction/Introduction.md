# Introduction

Welcome to the X11 Lab.

In this lab, we will use Rust to build a small X11 server step by step. The goal
is not to replace a production X server like Xorg, Xwayland, or XQuartz. The goal
is to understand how an X11 client talks to a display server by implementing the
important pieces ourselves.

X11 is a useful protocol to study because it is old, practical, network-aware,
and still visible in real systems. When a program such as `xclock`, `xeyes`, or
another graphical Linux application starts, it does not draw directly to your
screen. It connects to an X11 server, performs a setup handshake, creates
resources, opens windows, sends drawing requests, and receives events. By writing
the server side, we can make those ideas concrete.

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
