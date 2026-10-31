//! Network selection and registration (manual §4).
use crate::core::{Command, Error, formatted, literal};
pub fn registration() -> Command {
    literal("AT+CREG?")
}
pub fn operator() -> Command {
    literal("AT+COPS?")
}
pub fn system_info() -> Command {
    literal("AT+CPSI?")
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum RegistrationReporting {
    Disabled = 0,
    Status = 1,
    Location = 2,
}
pub fn registration_reporting(mode: RegistrationReporting) -> Command {
    formatted(format_args!("AT+CREG={}", mode as u8)).unwrap()
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum NetworkMode {
    Automatic = 2,
    Gsm = 13,
    Wcdma = 14,
    Lte = 38,
}
pub fn preferred_mode(mode: NetworkMode) -> Command {
    formatted(format_args!("AT+CNMP={}", mode as u8))
        .unwrap()
        .with_timeout(10_000)
        .unwrap()
}
pub fn automatic_time_update(enabled: bool) -> Command {
    formatted(format_args!("AT+CTZU={}", u8::from(enabled))).unwrap()
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RegistrationStatus {
    NotRegistered,
    Home,
    Searching,
    Denied,
    Unknown,
    Roaming,
    SmsOnlyHome,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Registration {
    pub reporting: Option<u8>,
    pub status: RegistrationStatus,
    pub location_area: Option<u16>,
    pub cell_id: Option<u32>,
}
impl Registration {
    /// `query` distinguishes +CREG: <n>,<stat> from unsolicited +CREG: <stat>.
    pub fn parse(line: &str, query: bool) -> Result<Self, Error> {
        let mut fields = line
            .trim()
            .strip_prefix("+CREG:")
            .ok_or(Error::InvalidArgument)?
            .split(',')
            .map(str::trim);
        let mut next = || fields.next().ok_or(Error::InvalidArgument);
        let reporting = if query {
            let n = next()?.parse::<u8>().map_err(|_| Error::InvalidArgument)?;
            if n > 2 {
                return Err(Error::InvalidArgument);
            }
            Some(n)
        } else {
            None
        };
        let status = match next()? {
            "0" => RegistrationStatus::NotRegistered,
            "1" => RegistrationStatus::Home,
            "2" => RegistrationStatus::Searching,
            "3" => RegistrationStatus::Denied,
            "4" => RegistrationStatus::Unknown,
            "5" => RegistrationStatus::Roaming,
            "6" => RegistrationStatus::SmsOnlyHome,
            _ => return Err(Error::InvalidArgument),
        };
        fn hex(s: &str) -> Result<u32, Error> {
            let s = s
                .strip_prefix('"')
                .and_then(|s| s.strip_suffix('"'))
                .ok_or(Error::InvalidArgument)?;
            u32::from_str_radix(s, 16).map_err(|_| Error::InvalidArgument)
        }
        let (location_area, cell_id) = match (fields.next(), fields.next()) {
            (None, None) => (None, None),
            (Some(lac), Some(ci)) => (
                Some(u16::try_from(hex(lac)?).map_err(|_| Error::InvalidArgument)?),
                Some(hex(ci)?),
            ),
            _ => return Err(Error::InvalidArgument),
        };
        if fields.next().is_some() {
            return Err(Error::InvalidArgument);
        }
        Ok(Self {
            reporting,
            status,
            location_area,
            cell_id,
        })
    }
    pub fn is_registered(self) -> bool {
        matches!(
            self.status,
            RegistrationStatus::Home | RegistrationStatus::Roaming
        )
    }
}
