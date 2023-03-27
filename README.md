# Custom Keyboard Firmware

![Keyboard Image](dactyl_manuform.jpg?raw=true "My crusty dactyl manuform keyboard.")

This is the code for a custom keyboard firmware that uses a matrix of keys to send keystrokes to a computer via USB. The firmware is designed to work with an STM32F407 microcontroller and uses the STM32F4xx HAL and USB libraries.

The firmware scans the keyboard matrix to determine which keys are being pressed and sends the corresponding keystrokes as a HID (Human Interface Device) report over USB to the host computer. The firmware also listens for HID reports from the host computer, which can be used to control the keyboard's behavior (e.g. to turn on/off certain modes or to remap keys), but this is not done, currently.

## Requirements

- STM32F407 microcontroller
- Rust toolchain
- Cargo-embed

## Usage

1. Connect the STM32F407 to the matrix of keys
2. Compile the firmware using `cargo build --release`
3. Load the firmware onto the STM32F407 using `cargo embed --release`

## Code Structure

### Dependencies

- `embedded_hal`: hardware abstraction layer for embedded systems
- `embedded_time`: time-keeping library for embedded systems
- `stm32f4xx_hal`: hardware abstraction layer for STM32F4xx microcontrollers
- `rtt_target`: library for printing to RTT (Real-Time Transfer) console

### Main Functionality

The firmware consists of two main functions:
- `iterate_lines()`: Scans the keyboard matrix and sends HID reports over USB
- `main()`: Initializes the RTT console and calls `iterate_lines()`

The `iterate_lines()` function does the following:
1. Initializes the STM32F407 peripherals and clocks
2. Initializes the USB bus
3. Scans the keyboard matrix for key presses
4. Sends HID reports over USB to the host computer
5. Listens for HID reports from the host computer

The firmware also includes a panic handler that prints error messages to the RTT console.

### Keyboard Mapping

The `keys` module defines two key mappings, `LEFT_KEYS` and `RIGHT_KEYS`, that map the physical key matrix to the corresponding HID keyboard codes. The `KeyMapping` enum and `mapping()` method are used to convert physical key presses to HID keyboard codes.

### Clock

The `DeviceClock` struct and `Clock` trait are used to provide a time source for the USB library. The `try_now()` method returns the current time as an `Instant` object.

### USB

The firmware uses the `usbd` library to implement a HID class USB device. The `UsbBusAllocator` is used to allocate endpoints for the USB device. The `NKROBootKeyboardInterface` is used to send HID reports over USB.

## Acknowledgements

https://medium.com/swlh/complete-idiot-guide-for-building-a-dactyl-manuform-keyboard-53454845b065