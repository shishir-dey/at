extern crate std;
use super::*;
#[cfg(feature = "a76xx")]
use crate::a76xx::*;
use crate::phy::Phy;
use std::{boxed::Box, collections::VecDeque, vec::Vec};

#[derive(Default)]
struct Mock {
    rx: VecDeque<u8>,
    tx: Vec<u8>,
    write_limit: Option<usize>,
    send_error: Option<i32>,
    receive_error: Option<i32>,
    bad_count: bool,
}
impl Mock {
    fn feed(&mut self, bytes: &[u8]) {
        self.rx.extend(bytes);
    }
}
impl Phy for Mock {
    fn send(&mut self, bytes: &[u8]) -> Result<usize, i32> {
        if let Some(e) = self.send_error {
            return Err(e);
        }
        if self.bad_count {
            return Ok(bytes.len() + 1);
        }
        let n = bytes.len().min(self.write_limit.unwrap_or(bytes.len()));
        self.tx.extend_from_slice(&bytes[..n]);
        Ok(n)
    }
    fn receive(&mut self, bytes: &mut [u8]) -> Result<usize, i32> {
        if let Some(e) = self.receive_error {
            return Err(e);
        }
        match self.rx.pop_front() {
            Some(b) => {
                bytes[0] = b;
                Ok(1)
            }
            None => Ok(0),
        }
    }
}
fn client() -> Client<Mock> {
    Client::new(Mock::default())
}
fn begin(c: &mut Client<Mock>, command: &Command) {
    c.start(command, 0).unwrap();
    assert_eq!(c.poll(0).unwrap(), None);
}

#[test]
#[cfg(feature = "a76xx")]
fn commands_follow_manual_and_reject_injection() {
    assert_eq!(call::dial("+12345").unwrap().as_bytes(), b"ATD+12345;\r");
    assert_eq!(
        sim::enter_pin("1234").unwrap().as_bytes(),
        b"AT+CPIN=\"1234\"\r"
    );
    assert_eq!(
        sms::send_text("+123").unwrap().as_bytes(),
        b"AT+CMGS=\"+123\"\r"
    );
    assert_eq!(sms::send_text("123").unwrap().timeout_ms(), 40_000);
    assert_eq!(
        network::preferred_mode(network::NetworkMode::Lte).as_bytes(),
        b"AT+CNMP=38\r"
    );
    assert_eq!(
        phonebook::write(1, "+123", "Alice").unwrap().as_bytes(),
        b"AT+CPBW=1,\"+123\",145,\"Alice\"\r"
    );
    assert_eq!(
        hardware::baud_rate(115200, true).unwrap().as_bytes(),
        b"AT+IPREX=115200\r"
    );
    assert_eq!(gnss::power(true).as_bytes(), b"AT+CGNSSPWR=1\r");
    assert!(Command::new("AT\rAT+CPOF").is_err());
    assert!(Command::new("AT;AT+CPOF").is_err());
    assert!(call::dial("123;AT+CPOF").is_err());
    assert!(phonebook::find("\"\rAT").is_err());
    assert!(sim::enter_pin("123").is_err());
    assert!(audio::microphone_gain(8).is_err());
    assert!(hardware::baud_rate(42, false).is_err());
    assert!(status::set_functionality(status::Functionality::RadioOff, true).is_err());
    assert!(Command::new(&std::format!("AT{}", "X".repeat(192))).is_err());
}
#[test]
fn partial_writes_echo_and_fragmented_responses() {
    let mut c = client();
    c.transport_mut().write_limit = Some(2);
    c.start(&Command::new("AT+CSQ").unwrap(), 0).unwrap();
    assert_eq!(c.start(&core::attention(), 0), Err(Error::Busy));
    for _ in 0..4 {
        assert_eq!(c.poll(1).unwrap(), None);
    }
    assert_eq!(c.transport_mut().tx, b"AT+CSQ\r");
    c.transport_mut().feed(b"AT+CSQ\r\n\r\n+CSQ: 31");
    assert_eq!(c.poll(2).unwrap(), None);
    c.transport_mut().feed(b",99\r\nOK\r\n");
    assert_eq!(c.poll(3).unwrap(), Some(Event::Line("+CSQ: 31,99")));
    assert_eq!(c.poll(4).unwrap(), Some(Event::Finished(ResultCode::Ok)));
    assert!(c.is_idle());
}
#[test]
fn preserves_urcs_before_and_after_completion() {
    let mut c = client();
    begin(&mut c, &Command::new("AT+CSQ").unwrap());
    c.transport_mut()
        .feed(b"RING\r\n+CREG: 1\r\n+CSQ: 10,0\r\nOK\r\n+CMTI: \"SM\",1\r\n");
    for expected in ["RING", "+CREG: 1", "+CSQ: 10,0"] {
        assert_eq!(c.poll(1).unwrap(), Some(Event::Line(expected)));
    }
    assert_eq!(c.poll(1).unwrap(), Some(Event::Finished(ResultCode::Ok)));
    assert_eq!(c.poll(1).unwrap(), Some(Event::Line("+CMTI: \"SM\",1")));
}
#[test]
fn error_results_retain_numeric_and_verbose_details() {
    for (response, expected) in [
        ("ERROR\r\n", ResultCode::Error),
        ("+CME ERROR: 10\r\n", ResultCode::CmeError("10")),
        (
            "+CMS ERROR: operation not allowed\r\n",
            ResultCode::CmsError("operation not allowed"),
        ),
    ] {
        let mut c = client();
        begin(&mut c, &core::attention());
        c.transport_mut().feed(response.as_bytes());
        assert_eq!(c.poll(1).unwrap(), Some(Event::Finished(expected)));
        assert!(c.is_idle());
    }
}
#[test]
#[cfg(feature = "a76xx")]
fn sms_prompt_body_and_completion() {
    let mut c = client();
    begin(&mut c, &sms::send_text("123").unwrap());
    c.transport_mut().feed(b"\r\n>");
    assert_eq!(c.poll(1).unwrap(), Some(Event::Prompt));
    assert_eq!(c.submit_sms(b"bad\x1a", 2), Err(Error::InvalidArgument));
    c.submit_sms(b"hello", 2).unwrap();
    c.poll(2).unwrap();
    assert_eq!(c.transport_mut().tx, b"AT+CMGS=\"123\"\rhello\x1a");
    c.transport_mut().feed(b" \r\n+CMGS: 46\r\nOK\r\n");
    assert_eq!(c.poll(3).unwrap(), Some(Event::Line("+CMGS: 46")));
    assert_eq!(c.poll(3).unwrap(), Some(Event::Finished(ResultCode::Ok)));
}
#[test]
#[cfg(feature = "a76xx")]
fn sms_cancel_and_unexpected_prompt() {
    let mut c = client();
    begin(&mut c, &sms::send_pdu(20).unwrap());
    c.transport_mut().feed(b"> ");
    assert_eq!(c.poll(1).unwrap(), Some(Event::Prompt));
    c.cancel_sms(2).unwrap();
    c.poll(2).unwrap();
    assert_eq!(c.transport_mut().tx.last(), Some(&0x1b));
    c.transport_mut().feed(b"\r\nOK\r\n");
    assert_eq!(c.poll(3).unwrap(), Some(Event::Finished(ResultCode::Ok)));
    assert_eq!(c.submit_sms(b"hello", 4), Err(Error::UnexpectedState));
    c.transport_mut().feed(b">ordinary line\r\n");
    assert_eq!(c.poll(4).unwrap(), Some(Event::Line(">ordinary line")));
}
#[test]
fn timeouts_include_stalled_writes_and_wraparound() {
    let mut c = client();
    c.transport_mut().write_limit = Some(0);
    c.start(&core::attention().with_timeout(20).unwrap(), u32::MAX - 9)
        .unwrap();
    assert_eq!(c.poll(9).unwrap(), None);
    assert_eq!(c.poll(10), Err(Error::Timeout));
    assert_eq!(c.start(&core::attention(), 11), Err(Error::NeedsRecovery));
    c.recover();
    assert!(c.is_idle());
    c.transport_mut().write_limit = None;
    begin(&mut c, &core::attention());
    assert_eq!(c.poll(9000), Err(Error::Timeout));
}
#[test]
fn bounded_buffers_and_malformed_input() {
    let mut c: Client<Mock, 4> = Client::new(Mock::default());
    assert_eq!(
        c.start(&Command::new("AT+CSQ").unwrap(), 0),
        Err(Error::BufferTooSmall)
    );
    c.start(&core::attention(), 0).unwrap();
    c.poll(0).unwrap();
    c.transport_mut().feed(b"12345\r\nOK\r\n");
    assert_eq!(c.poll(1), Err(Error::BufferTooSmall));
    assert_eq!(c.poll(2), Err(Error::NeedsRecovery));
    let mut c = client();
    c.transport_mut().feed(b"\xff\r\n");
    assert_eq!(c.poll(0), Err(Error::InvalidUtf8));
    let mut c: Client<Mock, 0> = Client::new(Mock::default());
    assert_eq!(c.start(&core::attention(), 0), Err(Error::BufferTooSmall));
}
#[test]
fn transport_errors_and_invalid_counts() {
    let mut c = client();
    c.transport_mut().send_error = Some(-7);
    c.start(&core::attention(), 0).unwrap();
    assert_eq!(c.poll(0), Err(Error::Transport(-7)));
    let mut c = client();
    c.transport_mut().bad_count = true;
    c.start(&core::attention(), 0).unwrap();
    assert_eq!(c.poll(0), Err(Error::InvalidTransportCount));
    let mut c = client();
    c.transport_mut().receive_error = Some(-8);
    assert_eq!(c.poll(0), Err(Error::Transport(-8)));
}
#[test]
fn boxed_and_borrowed_transports_forward_io() {
    let mut boxed: Box<dyn Phy> = Box::new(Mock::default());
    assert_eq!(boxed.send(b"AT\r"), Ok(3));
    let mut mock = Mock::default();
    mock.feed(b"x");
    let mut borrowed = &mut mock;
    let mut byte = [0];
    assert_eq!(Phy::receive(&mut borrowed, &mut byte), Ok(1));
    assert_eq!(byte, [b'x']);
}
#[test]
#[cfg(feature = "a76xx")]
fn parse_signal_registration_and_pin() {
    assert_eq!(
        status::SignalQuality::parse("+CSQ: 31,99").unwrap().dbm(),
        Some(-51)
    );
    assert_eq!(
        status::SignalQuality::parse("+CSQ: 99,99").unwrap().rssi,
        None
    );
    assert!(status::SignalQuality::parse("+CSQ: 32,0").is_err());
    assert!(status::SignalQuality::parse("+CSQ: 1,2,3").is_err());
    let r = network::Registration::parse("+CREG: 2,5,\"00C3\",\"12345678\"", true).unwrap();
    assert!(r.is_registered());
    assert_eq!(r.location_area, Some(195));
    assert_eq!(r.cell_id, Some(0x12345678));
    assert_eq!(
        network::Registration::parse("+CREG: 1", false)
            .unwrap()
            .reporting,
        None
    );
    assert!(network::Registration::parse("+CREG: 0,9", true).is_err());
    assert_eq!(
        sim::PinStatus::parse("+CPIN: READY"),
        Ok(sim::PinStatus::Ready)
    );
}
#[test]
#[cfg(feature = "a76xx")]
fn gps_manual_example_no_fix_and_bad_coordinates() {
    let fix =
        gnss::GpsFix::parse("+CGPSINFO:3113.343286,N,12121.234064,E,250311,072809.3,44.1,0.0,0")
            .unwrap()
            .unwrap();
    assert!((fix.latitude - 31.2223881).abs() < 0.0000001);
    assert!((fix.longitude - 121.3539010667).abs() < 0.0000001);
    assert_eq!(fix.altitude_m, Some(44.1));
    assert_eq!(gnss::GpsFix::parse("+CGPSINFO: ,,,,,,,,"), Ok(None));
    let fix = gnss::GpsFix::parse("+CGPSINFO:9000,S,18000,W,250311,072809.3,,,")
        .unwrap()
        .unwrap();
    assert_eq!(fix.latitude, -90.0);
    assert_eq!(fix.longitude, -180.0);
    for s in [
        "+CGPSINFO:9160,N,12100,E,250311,072809.3,,,",
        "+CGPSINFO:NaN,N,12100,E,250311,072809.3,,,",
        "+CGPSINFO:,,,,",
    ] {
        assert!(gnss::GpsFix::parse(s).is_err());
    }
}
