//! SMS configuration and sending (manual §9). Set the format before sending.
//! Prefer PDU mode for reading: text bodies can contain AT result-code-looking lines.
use crate::core::{Command, Error, formatted, literal, phone};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Format {
    Pdu = 0,
    Text = 1,
}
pub fn format(mode: Format) -> Command {
    formatted(format_args!("AT+CMGF={}", mode as u8)).unwrap()
}
pub fn current_format() -> Command {
    literal("AT+CMGF?")
}
pub fn service_center() -> Command {
    literal("AT+CSCA?")
}
/// Begins the prompt transaction; call Client::submit_sms only after Event::Prompt.
pub fn send_text(number: &str) -> Result<Command, Error> {
    phone(number)?;
    Ok(formatted(format_args!("AT+CMGS=\"{number}\""))?
        .with_timeout(40_000)?
        .with_prompt())
}
/// TPDU length in octets, excluding SMSC address bytes. Supply hex PDU at the prompt.
pub fn send_pdu(tpdu_octets: u16) -> Result<Command, Error> {
    if tpdu_octets == 0 {
        return Err(Error::InvalidArgument);
    }
    Ok(formatted(format_args!("AT+CMGS={tpdu_octets}"))?
        .with_timeout(40_000)?
        .with_prompt())
}
pub fn read(index: u16) -> Result<Command, Error> {
    if index == 0 {
        return Err(Error::InvalidArgument);
    }
    formatted(format_args!("AT+CMGR={index}"))
}
pub fn delete(index: u16) -> Result<Command, Error> {
    if index == 0 {
        return Err(Error::InvalidArgument);
    }
    formatted(format_args!("AT+CMGD={index}"))
}
/// Route new messages to storage and report +CMTI, avoiding inline text bodies.
pub fn storage_notifications() -> Command {
    literal("AT+CNMI=2,1,0,0,0")
}
