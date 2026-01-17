# Astral - Rail Power Monitor

This Rust application reads voltage and current from Astral GPUs from on-board ITE IT8915FN chip through NVAPI I2C communication protocol.

## Requirements

- Windows 10/11 (64-bit)
- ASUS Astral RTX 5080/5090 GPU (only 5090 tested)
- NVIDIA Driver installed
- Rust toolchain (stable)

## Building

```bash
cargo build --release
```

## Running

```bash
cargo run --release
```

## How it works

Astral 5080 and 5090 graphic cards from ASUS contain additional sensors, which can be accessed through on-board ITE IT8915FN chip via I2C protocol. We can use NVAPI for this, but the documented NvAPI_I2CRead only allows communication via DDC port. This is why we have to go through undocumented NvAPI_I2CReadEx, which can read via I2C port.

NvAPI is not supported on linux, but there is a better way to fetch data from the chip, because Linux kernel exposes some functions to communicate via I2C. Create an issue if you are interested in this.

## Register Information

Based on the SMBus dump:

- **Bus 4, Address 0x2B**: ITE IT8915FN
- **Registers 0x80-0x9F**: Current/Voltage monitoring data

24 bytes from register 0x80, organized as 6 rails with 4 bytes each (16-bit big-endian voltage + 16-bit big-endian current in millivolts and milliamps). The order appears to be reversed:

```
Byte Layout:
  0-1: Rail 0 Voltage    →  Pin 6 Voltage
  2-3: Rail 0 Current    →  Pin 6 Current
  4-5: Rail 1 Voltage    →  Pin 5 Voltage
  6-7: Rail 1 Current    →  Pin 5 Current
  8-9: Rail 2 Voltage    →  Pin 4 Voltage
10-11: Rail 2 Current    →  Pin 4 Current
12-13: Rail 3 Voltage    →  Pin 3 Voltage
14-15: Rail 3 Current    →  Pin 3 Current
16-17: Rail 4 Voltage    →  Pin 2 Voltage
18-19: Rail 4 Current    →  Pin 2 Current
20-21: Rail 5 Voltage    →  Pin 1 Voltage
22-23: Rail 5 Current    →  Pin 1 Current
```

## Future plans

- Expose or push data to Prometheus/InfluxDB
- GUI implementation
- Notifications, visual and audio alerts
- Shut down all programs using the graphics card

## References

- NVAPI I2C Documentation
- `ExpanModule.dll` reverse engineering
