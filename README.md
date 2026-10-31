# AT command library

A standalone `no_std` Cargo library. The generic client, command framing, result
codes, and `phy::Phy` transport are available without features. There are no
external dependencies or default features.

Enable SIMCom A76XX support explicitly:

```toml
[dependencies]
at = { path = "crates/at", features = ["a76xx"] }
```

The `a76xx` feature exposes modem-specific helpers under `at::a76xx` (for example,
`at::a76xx::gnss::gps_info()`). Standard commands remain under `at::core`.
Without this feature, the `a76xx` module is absent.


The `at` library with the `a76xx` feature implements the line protocol from the
A76XX Series AT Command Manual V1.06, sections 1–4, 6–11, and 24. It includes
command helpers for identity, status, registration, SIM PINs, voice calls, audio,
phonebook, SMS, UART control, and GNSS. `Command::new` supports additional single
AT commands; this is not a complete implementation of the manual's TCP/IP, HTTP,
MQTT, file-transfer, or binary protocols.

Implement `phy::Phy` for the board UART. Both methods must be nonblocking:
`Ok(0)` means no progress, partial reads/writes are allowed, and errors use the
transport's own integer codes. The client owns its transport (or borrows it via
`&mut`), uses fixed buffers, and does not allocate or require an async runtime.
The board UART and clock still need to be connected by the firmware application.

```rust
# #[cfg(feature = "a76xx")]
# mod example {
use at::{a76xx, Client, Event, ResultCode, phy::Phy};

fn query_signal<P: Phy>(uart: P, mut millis: impl FnMut() -> u32)
    -> Result<(), at::Error>
{
    let mut modem: Client<P> = Client::new(uart);
    modem.start(&a76xx::status::signal_quality(), millis())?;
    loop {
        match modem.poll(millis())? {
            Some(Event::Line(line)) if line.starts_with("+CSQ:") => {
                let quality = a76xx::status::SignalQuality::parse(line)?;
                let _dbm = quality.dbm();
            }
            Some(Event::Finished(ResultCode::Ok)) => break,
            Some(Event::Finished(_failure)) => {
                // Handle modem rejection separately from transport errors.
                break;
            }
            Some(Event::Line(_urc)) => { /* Dispatch unsolicited notifications. */ }
            _ => { /* Service other work while waiting. */ }
        }
    }
    Ok(())
}
# }
```

Use a monotonic, wrapping millisecond clock. Keep polling while idle for incoming
notifications. Events borrow the receive buffer until the next mutable access.
Only one command may be pending; response lines and URCs are delivered as `Line`
events, and final result codes as `Finished`. Use command context and prefixes to
interpret lines, especially registration queries versus unsolicited updates.

Start with a synchronized modem in command mode and the manual's default UART
settings (115200, 8N1, no flow control). Ensure `ATQ0` and `ATV1` are active, then
use `core::echo(false)` and `status::error_reporting(...)`. Numeric or suppressed
result codes cannot be used by this parser. Configure echo off before sending
SMS bodies. Changing modem baud rate also requires changing the host UART.

For SMS sending, select `sms::Format::Text` or `Pdu`, start `send_text` or
`send_pdu`, wait for `Event::Prompt`, then call `submit_sms` with an already encoded
body. The client appends CTRL-Z and waits for `+CMGS` and the final result. Use
`cancel_sms` at the prompt to send ESC. For reading, use PDU format: arbitrary
multiline text SMS bodies can contain strings such as `OK` and are not safely
framed by a general AT line parser. `storage_notifications` requests `+CMTI`
instead of unsolicited inline message bodies. Character encoding and SMS
segmentation are the caller's responsibility.

For GPS, enable `gnss::power(true)`, wait for the separate
`+CGNSSPWR: READY!` notification, then query `gnss::gps_info()`.
`GpsFix::parse` converts degrees/minutes to signed decimal coordinates and
returns `None` for an empty fix. Altitude is metres and speed is knots.

Commands are limited to 191 bytes before CR. Client TX and line buffers default
to 256 bytes each; choose a larger `Client<P, N>` for long PDU bodies or response
lines. Most helpers use the manual's 9-second timeout, SMS uses 40 seconds, and
network mode changes use 10 seconds. Dial/answer use conservative application
budgets of 120/60 seconds where the manual gives no maximum. Override budgets
with `Command::with_timeout`. The budget includes transmission; SMS submission
starts a fresh budget after the prompt.

A timeout, transport failure, invalid UTF-8, or overflowing line leaves the
client in a recovery-required state. Reset/resynchronize the modem and drain
stale UART input through `transport_mut()` before calling `recover()`. Merely
clearing client state cannot make late replies safe. A `CONNECT` result hands
data-mode handling to the caller; stop AT polling until command mode is restored.

Run the host protocol tests with `cargo test -p at --no-default-features` and
`cargo test -p at --features a76xx` and
check the library for the STM32C031 with
`cargo check -p at --target thumbv6m-none-eabi --features a76xx`.
