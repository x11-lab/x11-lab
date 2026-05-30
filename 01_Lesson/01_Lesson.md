# Lesson 01

Architecture:

```
WSL / Linux SSH session / Linux server
    runs xclock
        ↓
DISPLAY=<windows-ip>:0.0
        ↓
Rust app on Windows
    listens on TCP 6000
    accepts X11 handshake
    logs X11 requests
```

