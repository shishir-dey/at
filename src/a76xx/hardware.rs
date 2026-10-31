//! UART and hardware control (manual §10–11).
use crate::core::{Command, Error, formatted, literal};
/// The host must also change its UART speed. `permanent` writes the modem NVRAM.
pub fn baud_rate(baud: u32, permanent: bool) -> Result<Command, Error> {
    if ![
        300, 600, 1200, 2400, 4800, 9600, 19200, 38400, 57600, 115200, 230400, 460800, 921600,
        1842000, 3686400,
    ]
    .contains(&baud)
    {
        return Err(Error::InvalidArgument);
    }
    formatted(format_args!(
        "AT+{}={baud}",
        if permanent { "IPREX" } else { "IPR" }
    ))
}
pub fn flow_control(hardware: bool) -> Command {
    let mode = if hardware { 2 } else { 0 };
    formatted(format_args!("AT+IFC={mode},{mode}")).unwrap()
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum SleepMode {
    Disabled = 0,
    Dtr = 1,
    Rx = 2,
}
pub fn sleep(mode: SleepMode) -> Command {
    formatted(format_args!("AT+CSCLK={}", mode as u8)).unwrap()
}
pub fn supply_voltage() -> Command {
    literal("AT+CBC")
}
pub fn temperature() -> Command {
    literal("AT+CPMUTEMP")
}
