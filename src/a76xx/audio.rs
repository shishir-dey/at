//! Call audio (manual §7.2.17–21).
use crate::core::{Command, Error, formatted};
pub fn speaker_mute(muted: bool) -> Command {
    formatted(format_args!("AT+VMUTE={}", u8::from(muted))).unwrap()
}
pub fn microphone_mute(muted: bool) -> Command {
    formatted(format_args!("AT+CMUT={}", u8::from(muted))).unwrap()
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum VoiceDevice {
    Handset = 1,
    SpeakerPhone = 3,
}
pub fn voice_device(device: VoiceDevice) -> Command {
    formatted(format_args!("AT+CSDVC={}", device as u8)).unwrap()
}
pub fn microphone_gain(gain: u8) -> Result<Command, Error> {
    if gain > 7 {
        return Err(Error::InvalidArgument);
    }
    formatted(format_args!("AT+CMICGAIN={gain}"))
}
pub fn output_gain(gain: u8) -> Result<Command, Error> {
    if gain > 7 {
        return Err(Error::InvalidArgument);
    }
    formatted(format_args!("AT+COUTGAIN={gain}"))
}
