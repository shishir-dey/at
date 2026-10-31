//! Module status and control (manual §3).
use crate::core::{Command, Error, formatted, literal};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Functionality {
    Minimum = 0,
    Full = 1,
    RadioOff = 4,
    FactoryTest = 5,
    Reset = 6,
    Offline = 7,
}
/// Reset (6) requires Offline (7) first. Reset-before-setting is only valid for Full.
pub fn set_functionality(mode: Functionality, reset: bool) -> Result<Command, Error> {
    if reset && mode != Functionality::Full {
        return Err(Error::InvalidArgument);
    }
    formatted(format_args!("AT+CFUN={},{}", mode as u8, u8::from(reset)))
}
pub fn functionality() -> Command {
    literal("AT+CFUN?")
}
pub fn signal_quality() -> Command {
    literal("AT+CSQ")
}
pub fn activity() -> Command {
    literal("AT+CPAS")
}
pub fn clock() -> Command {
    literal("AT+CCLK?")
}
pub fn power_off() -> Command {
    literal("AT+CPOF")
}
pub fn reset() -> Command {
    literal("AT+CRESET")
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum ErrorReporting {
    Disabled = 0,
    Numeric = 1,
    Verbose = 2,
}
pub fn error_reporting(mode: ErrorReporting) -> Command {
    formatted(format_args!("AT+CMEE={}", mode as u8)).unwrap()
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SignalQuality {
    pub rssi: Option<u8>,
    pub ber: Option<u8>,
}
impl SignalQuality {
    pub fn parse(line: &str) -> Result<Self, Error> {
        let (rssi, ber) = line
            .trim()
            .strip_prefix("+CSQ:")
            .ok_or(Error::InvalidArgument)?
            .trim()
            .split_once(',')
            .ok_or(Error::InvalidArgument)?;
        fn value(s: &str, max: u8) -> Result<Option<u8>, Error> {
            match s.trim().parse::<u8>().map_err(|_| Error::InvalidArgument)? {
                99 => Ok(None),
                n if n <= max => Ok(Some(n)),
                _ => Err(Error::InvalidArgument),
            }
        }
        Ok(Self {
            rssi: value(rssi, 31)?,
            ber: value(ber, 7)?,
        })
    }
    /// Endpoints are bounds: RSSI 0 means <= -113 dBm, 31 means >= -51 dBm.
    pub fn dbm(self) -> Option<i16> {
        self.rssi.map(|r| -113 + 2 * i16::from(r))
    }
}
