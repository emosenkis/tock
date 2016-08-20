use core::cell::Cell;
use common::take_cell::TakeCell;
use main::{AppId, Callback, Driver};
use hil::i2c;
use hil;

// Buffer to use for I2C messages
pub static mut BUFFER : [u8; 4] = [0; 4];

#[allow(dead_code)]
enum Registers {
    IoDir = 0x00,
    IPol = 0x01,
    GpIntEn = 0x02,
    DefVal = 0x03,
    IntCon = 0x04,
    IoCon = 0x05,
    GpPu = 0x06,
    IntF = 0x07,
    IntCap = 0x08,
    Gpio = 0x09,
    OLat = 0x0a,
}

/// States of the I2C protocol with the MCP23008.
#[derive(Clone,Copy,PartialEq)]
enum State {
    Idle,

    SelectIoDir,
    ReadIoDir,
    SelectGpPu,
    ReadGpPu,
    SelectGpio,
    ReadGpio,
    SelectGpioToggle,
    ReadGpioToggle,
    SelectGpioRead,
    ReadGpioRead,

    /// Disable I2C and release buffer
    Done,
}

enum Direction {
    Input = 0x01,
    Output = 0x00,
}

enum PinState {
    High = 0x01,
    Low = 0x00,
}

pub struct MCP23008<'a> {
    i2c: &'a i2c::I2CDevice,
    callback: Cell<Option<Callback>>,
    state: Cell<State>,
    buffer: TakeCell<&'static mut [u8]>
}

impl<'a> MCP23008<'a> {
    pub fn new(i2c: &'a i2c::I2CDevice, buffer: &'static mut [u8]) -> MCP23008<'a> {
        // setup and return struct
        MCP23008{
            i2c: i2c,
            callback: Cell::new(None),
            state: Cell::new(State::Idle),
            buffer: TakeCell::new(buffer)
        }
    }

    fn set_direction(&self, pin_number: u8, direction: Direction) {
        self.buffer.take().map(|buffer| {
            // turn on i2c to send commands
            self.i2c.enable();

            buffer[0] = Registers::IoDir as u8;
            // Save settings in buffer so they automatically get passed to
            // state machine.
            buffer[1] = pin_number;
            buffer[2] = direction as u8;
            self.i2c.write(buffer, 1);
            self.state.set(State::SelectIoDir);
        });
    }

    fn configure_pullup(&self, pin_number: u8, enabled: bool) {
        self.buffer.take().map(|buffer| {
            // turn on i2c to send commands
            self.i2c.enable();

            buffer[0] = Registers::GpPu as u8;
            // Save settings in buffer so they automatically get passed to
            // state machine.
            buffer[1] = pin_number;
            buffer[2] = enabled as u8;
            self.i2c.write(buffer, 1);
            self.state.set(State::SelectGpPu);
        });
    }

    fn set_pin(&self, pin_number: u8, value: PinState) {
        self.buffer.take().map(|buffer| {
            // turn on i2c to send commands
            self.i2c.enable();

            buffer[0] = Registers::Gpio as u8;
            // Save settings in buffer so they automatically get passed to
            // state machine.
            buffer[1] = pin_number;
            buffer[2] = value as u8;
            self.i2c.write(buffer, 1);
            self.state.set(State::SelectGpio);
        });
    }

    fn toggle_pin(&self, pin_number: u8) {
        self.buffer.take().map(|buffer| {
            // turn on i2c to send commands
            self.i2c.enable();

            buffer[0] = Registers::Gpio as u8;
            // Save settings in buffer so they automatically get passed to
            // state machine.
            buffer[1] = pin_number;
            self.i2c.write(buffer, 1);
            self.state.set(State::SelectGpioToggle);
        });
    }

    fn read_pin(&self, pin_number: u8) -> bool {
        self.buffer.take().map(|buffer| {
            // turn on i2c to send commands
            self.i2c.enable();

            buffer[0] = Registers::Gpio as u8;
            // Save settings in buffer so they automatically get passed to
            // state machine.
            buffer[1] = pin_number;
            self.i2c.write(buffer, 1);
            self.state.set(State::SelectGpioRead);
        });

        // TODO: not sure how to fix this!!!
        false
    }

}

impl<'a> i2c::I2CClient for MCP23008<'a> {
    fn command_complete(&self, buffer: &'static mut [u8], _error: i2c::Error) {
        match self.state.get() {
            State::SelectIoDir => {
                self.i2c.read(buffer, 1);
                self.state.set(State::ReadIoDir);
            },
            State::ReadIoDir => {
                let pin_number = buffer[1];
                let direction = buffer[2];
                if direction == Direction::Input as u8 {
                    buffer[1] = buffer[0] | (1 << pin_number);
                } else {
                    buffer[1] = buffer[0] & !(1 << pin_number);
                }
                buffer[0] = Registers::IoDir as u8;
                self.i2c.write(buffer, 2);
                self.state.set(State::Done);
            },
            State::SelectGpPu => {
                self.i2c.read(buffer, 1);
                self.state.set(State::ReadGpPu);
            },
            State::ReadGpPu => {
                let pin_number = buffer[1];
                let enabled = buffer[2] == 1;
                if enabled  {
                    buffer[1] = buffer[0] | (1 << pin_number);
                } else {
                    buffer[1] = buffer[0] & !(1 << pin_number);
                }
                buffer[0] = Registers::GpPu as u8;
                self.i2c.write(buffer, 2);
                self.state.set(State::Done);
            },
            State::SelectGpio => {
                self.i2c.read(buffer, 1);
                self.state.set(State::ReadGpio);
            },
            State::ReadGpio => {
                let pin_number = buffer[1];
                let value = buffer[2];
                if value == PinState::High as u8 {
                    buffer[1] = buffer[0] | (1 << pin_number);
                } else {
                    buffer[1] = buffer[0] & !(1 << pin_number);
                }
                buffer[0] = Registers::Gpio as u8;
                self.i2c.write(buffer, 2);
                self.state.set(State::Done);
            },
            State::SelectGpioToggle => {
                self.i2c.read(buffer, 1);
                self.state.set(State::ReadGpioToggle);
            },
            State::ReadGpioToggle => {
                let pin_number = buffer[1];
                buffer[1] = buffer[0] ^ (1 << pin_number);
                buffer[0] = Registers::Gpio as u8;
                self.i2c.write(buffer, 2);
                self.state.set(State::Done);
            },
            State::SelectGpioRead => {
                self.i2c.read(buffer, 1);
                self.state.set(State::ReadGpioRead);
            },
            State::ReadGpioRead => {
                let pin_number = buffer[1];
                let pin_value = (buffer[0] >> pin_number) & 0x01;

                // Todo: do something with pin_value

                self.buffer.replace(buffer);
                self.i2c.disable();
                self.state.set(State::Idle);
            },
            // State::SelectElectronicId2 => {
            //     self.i2c.read(buffer, 6);
            //     self.state.set(State::ReadElectronicId2);
            // },
            // State::ReadElectronicId2 => {
            //     self.buffer.replace(buffer);
            //     self.i2c.disable();
            //     self.state.set(State::Idle);
            // },
            // State::TakeMeasurementInit => {

            //     let interval = (20 as u32) * <A::Frequency>::frequency() / 1000;

            //     let now = self.alarm.now();
            //     let tics = self.alarm.now().wrapping_add(interval);
            //     self.alarm.set_alarm(tics);

            //     // Now wait for timer to expire
            //     self.buffer.replace(buffer);
            //     self.i2c.disable();
            //     self.state.set(State::Idle);
            // },
            // State::ReadRhMeasurement => {
            //     buffer[2] = buffer[0];
            //     buffer[3] = buffer[1];
            //     buffer[0] = Registers::ReadTemperaturePreviousRHMeasurement as u8;
            //     self.i2c.write(buffer, 1);
            //     self.state.set(State::ReadTempMeasurement);
            // },
            // State::ReadTempMeasurement => {
            //     self.i2c.read(buffer, 2);
            //     self.state.set(State::GotMeasurement);
            // },
            // State::GotMeasurement => {

            //     // Temperature in hundredths of degrees centigrade
            //     let temp_raw = (((buffer[0] as u32) << 8) | (buffer[1] as u32)) as u32;
            //     let temp = (((temp_raw * 17572) / 65536) - 4685) as i16;

            //     // Humidity in hundredths of percent
            //     let humidity_raw = (((buffer[2] as u32) << 8) | (buffer[3] as u32)) as u32;
            //     let humidity = (((humidity_raw * 125 * 100) / 65536) - 600) as u16;

            //     self.callback.get().map(|mut cb|
            //         cb.schedule(temp as usize, humidity as usize, 0)
            //     );

            //     self.buffer.replace(buffer);
            //     self.i2c.disable();
            //     self.state.set(State::Idle);
            // },
            State::Done => {
                self.buffer.replace(buffer);
                self.i2c.disable();
                self.state.set(State::Idle);
            },
            _ => {}
        }
    }
}


pub struct GPIOPin<'a> {
    mcp23008: &'a MCP23008<'a>,
    pin: u8,
    callback: Cell<Option<Callback>>,
}

impl<'a> GPIOPin<'a> {
    pub fn new(mcp23008: &'a MCP23008, pin: u8) -> GPIOPin<'a> {
        // setup and return struct
        GPIOPin{
            mcp23008: mcp23008,
            pin: pin,
            callback: Cell::new(None),
        }
    }
}

impl<'a> hil::gpio::BroadInterface for GPIOPin<'a> {

    fn set_client(&self, client: &'static hil::gpio::Client) {
        // self.client.replace(client);
    }
}

impl<'a> hil::gpio::GPIOPin for GPIOPin<'a> {
    fn disable(&self) {
        // ??
    }

    fn enable_output(&self) {
        self.mcp23008.set_direction(self.pin, Direction::Output);
    }

    fn enable_input(&self, mode: hil::gpio::InputMode) {
        self.mcp23008.set_direction(self.pin, Direction::Input);
        match mode {
            hil::gpio::InputMode::PullUp => {
                self.mcp23008.configure_pullup(self.pin, true);
            },
            hil::gpio::InputMode::PullDown => {
                // No support for this
            },
            hil::gpio::InputMode::PullNone => {
                self.mcp23008.configure_pullup(self.pin, false);
            },
        }
    }

    fn read(&self) -> bool {
        self.mcp23008.read_pin(self.pin)
    }

    fn toggle(&self) {
        self.mcp23008.toggle_pin(self.pin);
    }

    fn set(&self) {
        self.mcp23008.set_pin(self.pin, PinState::High);
    }

    fn clear(&self) {
        self.mcp23008.set_pin(self.pin, PinState::Low);
    }

    fn enable_interrupt(&self, client_data: usize,
                        mode: hil::gpio::InterruptMode) {
        // not yet implemented
    }

    fn disable_interrupt(&self) {
        // not yet implemented
    }
}





// impl<'a, A: alarm::Alarm + 'a> alarm::AlarmClient for SI7021<'a, A> {
//     fn fired(&self) {
//         self.buffer.take().map(|buffer| {
//             // turn on i2c to send commands
//             self.i2c.enable();

//             self.i2c.read(buffer, 2);
//             self.state.set(State::ReadRhMeasurement);
//         });
//     }
// }

// impl<'a, A: alarm::Alarm + 'a> Driver for SI7021<'a, A> {
//     fn subscribe(&self, subscribe_num: usize, callback: Callback) -> isize {
//         match subscribe_num {
//             // Set a callback
//             0 => {
//                 // Set callback function
//                 self.callback.set(Some(callback));
//                 0
//             },
//             // default
//             _ => -1
//         }
//     }

//     fn command(&self, command_num: usize, _: usize, _: AppId) -> isize {
//         match command_num {
//             // Take a pressure measurement
//             0 => {
//                 self.take_measurement();
//                 0
//             },
//             // default
//             _ => -1
//         }

//     }
// }
