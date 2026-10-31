//! SIM identity and PIN handling (manual §6).
use crate::core::{Command, Error, formatted, literal};
pub fn iccid() -> Command {
    literal("AT+CICCID")
}
pub fn imsi() -> Command {
    literal("AT+CIMI")
}
pub fn pin_status() -> Command {
    literal("AT+CPIN?")
}
pub fn service_provider() -> Command {
    literal("AT+CSPN?")
}
fn pin_valid(pin: &str) -> bool {
    (4..=8).contains(&pin.len()) && pin.bytes().all(|b| b.is_ascii_digit())
}
pub fn enter_pin(pin: &str) -> Result<Command, Error> {
    if !pin_valid(pin) {
        return Err(Error::InvalidArgument);
    }
    formatted(format_args!("AT+CPIN=\"{pin}\""))
}
pub fn unlock_puk(puk: &str, new_pin: &str) -> Result<Command, Error> {
    if puk.len() != 8 || !pin_valid(puk) || !pin_valid(new_pin) {
        return Err(Error::InvalidArgument);
    }
    formatted(format_args!("AT+CPIN=\"{puk}\",\"{new_pin}\""))
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PinStatus<'a> {
    Ready,
    SimPin,
    SimPuk,
    SimPin2,
    SimPuk2,
    Other(&'a str),
}
impl<'a> PinStatus<'a> {
    pub fn parse(line: &'a str) -> Result<Self, Error> {
        Ok(
            match line
                .trim()
                .strip_prefix("+CPIN:")
                .ok_or(Error::InvalidArgument)?
                .trim()
            {
                "READY" => Self::Ready,
                "SIM PIN" => Self::SimPin,
                "SIM PUK" => Self::SimPuk,
                "SIM PIN2" => Self::SimPin2,
                "SIM PUK2" => Self::SimPuk2,
                "" => return Err(Error::InvalidArgument),
                s => Self::Other(s),
            },
        )
    }
}
