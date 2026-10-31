//! SIM phonebook (manual §8). Query CPBR/CPBW test forms for device limits.
use crate::core::{Command, Error, formatted, literal, phone, quoted};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Storage {
    Sim,
    FixedDialing,
    OwnNumbers,
    Application,
}
pub fn select_storage(storage: Storage) -> Command {
    let name = match storage {
        Storage::Sim => "SM",
        Storage::FixedDialing => "FD",
        Storage::OwnNumbers => "ON",
        Storage::Application => "AP",
    };
    formatted(format_args!("AT+CPBS=\"{name}\"")).unwrap()
}
pub fn storage() -> Command {
    literal("AT+CPBS?")
}
pub fn subscriber_number() -> Command {
    literal("AT+CNUM")
}
pub fn read(first: u16, last: u16) -> Result<Command, Error> {
    if first == 0 || last < first {
        return Err(Error::InvalidArgument);
    }
    formatted(format_args!("AT+CPBR={first},{last}"))
}
pub fn find(text: &str) -> Result<Command, Error> {
    quoted(text)?;
    formatted(format_args!("AT+CPBF=\"{text}\""))
}
pub fn write(index: u16, number: &str, name: &str) -> Result<Command, Error> {
    if index == 0 || name.len() > 14 {
        return Err(Error::InvalidArgument);
    }
    phone(number)?;
    quoted(name)?;
    let kind = if number.starts_with('+') { 145 } else { 129 };
    formatted(format_args!(
        "AT+CPBW={index},\"{number}\",{kind},\"{name}\""
    ))
}
pub fn delete(index: u16) -> Result<Command, Error> {
    if index == 0 {
        return Err(Error::InvalidArgument);
    }
    formatted(format_args!("AT+CPBW={index}"))
}
