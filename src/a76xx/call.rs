//! Voice calling (manual §2.2.1–4 and §7).
use crate::core::{Command, Error, formatted, literal, phone};
/// Voice dial; the terminating semicolon is required by the modem.
pub fn dial(number: &str) -> Result<Command, Error> {
    phone(number)?;
    formatted(format_args!("ATD{number};"))?.with_timeout(120_000)
}
pub fn answer() -> Command {
    literal("ATA").with_timeout(60_000).unwrap()
}
pub fn hang_up() -> Command {
    literal("AT+CHUP")
}
pub fn current_calls() -> Command {
    literal("AT+CLCC")
}
pub fn last_error() -> Command {
    literal("AT+CEER")
}
pub fn auto_answer(rings: u8) -> Command {
    formatted(format_args!("ATS0={rings}")).unwrap()
}
pub fn caller_id(enabled: bool) -> Command {
    formatted(format_args!("AT+CLIP={}", u8::from(enabled))).unwrap()
}
