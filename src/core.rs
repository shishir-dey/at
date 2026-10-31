//! AT line protocol (manual §1.5 and §2). No allocation, timer, or UART dependency.
use crate::phy::Phy;
#[cfg(feature = "a76xx")]
use ::core::fmt::{self, Write};

pub const DEFAULT_TIMEOUT_MS: u32 = 9_000;
pub const COMMAND_CAPACITY: usize = 192;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    InvalidArgument,
    BufferTooSmall,
    Busy,
    Timeout,
    Transport(i32),
    InvalidTransportCount,
    InvalidUtf8,
    NeedsRecovery,
    UnexpectedState,
}

/// A single CR-terminated command. Debug is intentionally omitted (may contain a PIN).
#[derive(Clone)]
pub struct Command {
    bytes: [u8; COMMAND_CAPACITY],
    len: usize,
    timeout_ms: u32,
    prompt: bool,
}
impl Command {
    /// Escape hatch for commands not covered by the typed helpers. Excludes CR/LF,
    /// control characters and command chaining; framing is added here.
    pub fn new(text: &str) -> Result<Self, Error> {
        if !text.starts_with("AT") || !text.bytes().all(|b| (32..=126).contains(&b)) {
            return Err(Error::InvalidArgument);
        }
        // A final semicolon is needed for voice dial commands.
        if text.contains(';')
            && !(text.starts_with("ATD")
                && text.ends_with(';')
                && !text[..text.len() - 1].contains(';'))
        {
            return Err(Error::InvalidArgument);
        }
        if text.len() >= COMMAND_CAPACITY {
            return Err(Error::BufferTooSmall);
        }
        let mut result = Self {
            bytes: [0; COMMAND_CAPACITY],
            len: text.len() + 1,
            timeout_ms: DEFAULT_TIMEOUT_MS,
            prompt: false,
        };
        result.bytes[..text.len()].copy_from_slice(text.as_bytes());
        result.bytes[text.len()] = b'\r';
        Ok(result)
    }
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes[..self.len]
    }
    pub fn timeout_ms(&self) -> u32 {
        self.timeout_ms
    }
    pub fn with_timeout(mut self, ms: u32) -> Result<Self, Error> {
        if ms == 0 || ms > i32::MAX as u32 {
            return Err(Error::InvalidArgument);
        }
        self.timeout_ms = ms;
        Ok(self)
    }
    #[cfg(feature = "a76xx")]
    pub(crate) fn with_prompt(mut self) -> Self {
        self.prompt = true;
        self
    }
}

#[cfg(feature = "a76xx")]
pub(crate) fn formatted(args: fmt::Arguments<'_>) -> Result<Command, Error> {
    struct Buffer {
        bytes: [u8; COMMAND_CAPACITY],
        len: usize,
    }
    impl Write for Buffer {
        fn write_str(&mut self, s: &str) -> fmt::Result {
            if self.len + s.len() >= self.bytes.len() {
                return Err(fmt::Error);
            }
            self.bytes[self.len..self.len + s.len()].copy_from_slice(s.as_bytes());
            self.len += s.len();
            Ok(())
        }
    }
    let mut buf = Buffer {
        bytes: [0; COMMAND_CAPACITY],
        len: 0,
    };
    buf.write_fmt(args).map_err(|_| Error::BufferTooSmall)?;
    Command::new(::core::str::from_utf8(&buf.bytes[..buf.len]).map_err(|_| Error::InvalidArgument)?)
}
pub(crate) fn literal(s: &str) -> Command {
    Command::new(s).expect("valid built-in AT command")
}
#[cfg(feature = "a76xx")]
pub(crate) fn quoted(s: &str) -> Result<(), Error> {
    if s.bytes()
        .all(|b| (32..=126).contains(&b) && !matches!(b, b'"' | b'\\' | b';'))
    {
        Ok(())
    } else {
        Err(Error::InvalidArgument)
    }
}
#[cfg(feature = "a76xx")]
pub(crate) fn phone(s: &str) -> Result<(), Error> {
    let digits = s.strip_prefix('+').unwrap_or(s);
    if !digits.is_empty()
        && digits
            .bytes()
            .all(|b| b.is_ascii_digit() || b == b'*' || b == b'#')
    {
        Ok(())
    } else {
        Err(Error::InvalidArgument)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResultCode<'a> {
    Ok,
    Error,
    CmeError(&'a str),
    CmsError(&'a str),
    NoCarrier,
    Busy,
    NoAnswer,
    NoDialTone,
    Connect,
}
fn result_code(line: &str) -> Option<ResultCode<'_>> {
    Some(match line {
        "OK" => ResultCode::Ok,
        "ERROR" => ResultCode::Error,
        "NO CARRIER" => ResultCode::NoCarrier,
        "BUSY" => ResultCode::Busy,
        "NO ANSWER" => ResultCode::NoAnswer,
        "NO DIALTONE" => ResultCode::NoDialTone,
        "CONNECT" => ResultCode::Connect,
        _ if line.starts_with("CONNECT ") => ResultCode::Connect,
        _ if line.starts_with("+CME ERROR:") => ResultCode::CmeError(line[11..].trim()),
        _ if line.starts_with("+CMS ERROR:") => ResultCode::CmsError(line[11..].trim()),
        _ => return None,
    })
}
#[derive(Debug, PartialEq, Eq)]
pub enum Event<'a> {
    /// Information response or URC. Both are delivered, including while idle.
    /// The caller uses the current command and line prefix to interpret it.
    Line(&'a str),
    Prompt,
    Finished(ResultCode<'a>),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum State {
    Idle,
    Sending,
    Waiting,
    WaitingPrompt,
    AwaitingBody,
    SendingBody,
    Fault,
}

/// Poll-driven client for verbose (ATV1), enabled (ATQ0) result codes.
/// Keep polling while idle to receive URCs. Transport methods MUST be nonblocking.
/// On timeout/I/O/framing failure, reset/drain the modem before `recover`.
/// Raw binary/data mode and arbitrary multiline SMS bodies are not line protocols
/// and must be handled separately. A CONNECT result transfers control to the caller.
pub struct Client<P, const N: usize = 256> {
    phy: P,
    tx: [u8; N],
    tx_len: usize,
    sent: usize,
    line: [u8; N],
    line_len: usize,
    state: State,
    started: u32,
    timeout: u32,
    prompt: bool,
    skip_prompt_space: bool,
}
impl<P: Phy, const N: usize> Client<P, N> {
    pub fn new(phy: P) -> Self {
        Self {
            phy,
            tx: [0; N],
            tx_len: 0,
            sent: 0,
            line: [0; N],
            line_len: 0,
            state: State::Idle,
            started: 0,
            timeout: DEFAULT_TIMEOUT_MS,
            prompt: false,
            skip_prompt_space: false,
        }
    }
    pub fn is_idle(&self) -> bool {
        self.state == State::Idle
    }
    pub fn into_inner(self) -> P {
        self.phy
    }
    /// Access for board-level recovery. Do not interleave independent AT traffic.
    pub fn transport_mut(&mut self) -> &mut P {
        &mut self.phy
    }
    /// Call ONLY after external reset/resynchronization and draining stale RX bytes.
    pub fn recover(&mut self) {
        self.state = State::Idle;
        self.line_len = 0;
        self.tx_len = 0;
        self.skip_prompt_space = false;
    }
    pub fn start(&mut self, command: &Command, now_ms: u32) -> Result<(), Error> {
        if self.state == State::Fault {
            return Err(Error::NeedsRecovery);
        }
        if !self.is_idle() || self.line_len != 0 {
            return Err(Error::Busy);
        }
        if command.len > N {
            return Err(Error::BufferTooSmall);
        }
        self.tx[..command.len].copy_from_slice(command.as_bytes());
        self.tx_len = command.len;
        self.sent = 0;
        self.started = now_ms;
        self.timeout = command.timeout_ms;
        self.prompt = command.prompt;
        self.state = State::Sending;
        Ok(())
    }
    /// After Prompt, submit an already encoded SMS body/PDU, followed by CTRL-Z.
    /// No character-set conversion is performed. ESC and CTRL-Z in the body are rejected.
    pub fn submit_sms(&mut self, body: &[u8], now_ms: u32) -> Result<(), Error> {
        if self.state != State::AwaitingBody {
            return Err(Error::UnexpectedState);
        }
        if body.iter().any(|b| matches!(b, 0x1a | 0x1b)) {
            return Err(Error::InvalidArgument);
        }
        if body.len() >= N {
            return Err(Error::BufferTooSmall);
        }
        self.tx[..body.len()].copy_from_slice(body);
        self.tx[body.len()] = 0x1a;
        self.tx_len = body.len() + 1;
        self.sent = 0;
        self.started = now_ms;
        self.state = State::SendingBody;
        Ok(())
    }
    pub fn cancel_sms(&mut self, now_ms: u32) -> Result<(), Error> {
        if self.state != State::AwaitingBody {
            return Err(Error::UnexpectedState);
        }
        if N == 0 {
            return Err(Error::BufferTooSmall);
        }
        self.tx[0] = 0x1b;
        self.tx_len = 1;
        self.sent = 0;
        self.started = now_ms;
        self.state = State::SendingBody;
        Ok(())
    }
    fn fail<T>(&mut self, error: Error) -> Result<T, Error> {
        self.state = State::Fault;
        Err(error)
    }
    /// At most one write or 64 single-byte reads per call. `now_ms` is a wrapping,
    /// monotonic millisecond clock, polled more often than once per clock wrap.
    pub fn poll(&mut self, now_ms: u32) -> Result<Option<Event<'_>>, Error> {
        if self.state == State::Fault {
            return Err(Error::NeedsRecovery);
        }
        if !self.is_idle() && now_ms.wrapping_sub(self.started) >= self.timeout {
            return self.fail(Error::Timeout);
        }
        if matches!(self.state, State::Sending | State::SendingBody) {
            let count = match self.phy.send(&self.tx[self.sent..self.tx_len]) {
                Ok(n) => n,
                Err(e) => return self.fail(Error::Transport(e)),
            };
            if count > self.tx_len - self.sent {
                return self.fail(Error::InvalidTransportCount);
            }
            self.sent += count;
            if self.sent == self.tx_len {
                self.state = if self.state == State::Sending && self.prompt {
                    State::WaitingPrompt
                } else {
                    State::Waiting
                };
            }
            return Ok(None);
        }
        for _ in 0..64 {
            let mut byte = [0];
            match self.phy.receive(&mut byte) {
                Ok(0) => return Ok(None),
                Ok(1) => {}
                Ok(_) => return self.fail(Error::InvalidTransportCount),
                Err(e) => return self.fail(Error::Transport(e)),
            }
            let b = byte[0];
            if self.skip_prompt_space {
                self.skip_prompt_space = false;
                if b == b' ' {
                    continue;
                }
            }
            if self.state == State::WaitingPrompt && b == b'>' && self.line_len == 0 {
                self.state = State::AwaitingBody;
                self.skip_prompt_space = true;
                return Ok(Some(Event::Prompt));
            }
            if b == b'\r' || b == b'\n' {
                if self.line_len == 0 {
                    continue;
                }
                let len = self.line_len;
                self.line_len = 0;
                // Command echo is not an information response.
                if self.tx_len > 0
                    && self.tx[self.tx_len - 1] == b'\r'
                    && self.line[..len] == self.tx[..self.tx_len - 1]
                {
                    continue;
                }
                if ::core::str::from_utf8(&self.line[..len]).is_err() {
                    return self.fail(Error::InvalidUtf8);
                }
                let line = ::core::str::from_utf8(&self.line[..len]).unwrap();
                if matches!(
                    self.state,
                    State::Waiting | State::WaitingPrompt | State::AwaitingBody
                ) && let Some(code) = result_code(line.trim())
                {
                    self.state = State::Idle;
                    self.tx_len = 0;
                    return Ok(Some(Event::Finished(code)));
                }
                return Ok(Some(Event::Line(line)));
            }
            if self.line_len == N {
                return self.fail(Error::BufferTooSmall);
            }
            self.line[self.line_len] = b;
            self.line_len += 1;
        }
        Ok(None)
    }
}

pub fn attention() -> Command {
    literal("AT")
}
pub fn echo(enabled: bool) -> Command {
    literal(if enabled { "ATE1" } else { "ATE0" })
}
pub fn verbose_results() -> Command {
    literal("ATV1")
}
pub fn enable_results() -> Command {
    literal("ATQ0")
}
pub fn identification() -> Command {
    literal("ATI")
}
pub fn manufacturer() -> Command {
    literal("AT+CGMI")
}
pub fn model() -> Command {
    literal("AT+CGMM")
}
pub fn firmware_revision() -> Command {
    literal("AT+CGMR")
}
pub fn imei() -> Command {
    literal("AT+CGSN")
}
pub fn save_settings() -> Command {
    literal("AT&W")
}
pub fn restore_settings() -> Command {
    literal("ATZ")
}
pub fn factory_defaults() -> Command {
    literal("AT&F")
}
