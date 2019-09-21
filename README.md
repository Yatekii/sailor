# Sailor

A sailing navigation application.

![screenshot](doc/img/screenshot.png)

## Building

### Building for vulkan & Linux/Windows

```
cargo build --verbose --bin sailor
```

### Building for metal & macOS

```
cargo build --verbose --bin sailor --no-default-features --features metal
```