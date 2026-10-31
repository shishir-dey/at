//! GNSS commands (manual §24). Wait for +CGNSSPWR: READY! before querying fixes.
use crate::core::{Command, Error, formatted, literal};
pub fn power(enabled: bool) -> Command {
    formatted(format_args!("AT+CGNSSPWR={}", u8::from(enabled))).unwrap()
}
pub fn power_status() -> Command {
    literal("AT+CGNSSPWR?")
}
pub fn gps_info() -> Command {
    literal("AT+CGPSINFO")
}
pub fn gnss_info() -> Command {
    literal("AT+CGNSSINFO")
}
pub fn report_interval(seconds: u8) -> Command {
    formatted(format_args!("AT+CGPSINFO={seconds}")).unwrap()
}
pub fn cold_start() -> Command {
    literal("AT+CGPSCOLD")
}
pub fn warm_start() -> Command {
    literal("AT+CGPSWARM")
}
pub fn hot_start() -> Command {
    literal("AT+CGPSHOT")
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GpsFix<'a> {
    pub latitude: f64,
    pub longitude: f64,
    /// Raw ddmmyy and hhmmss.s UTC fields from the module.
    pub date: &'a str,
    pub utc_time: &'a str,
    pub altitude_m: Option<f64>,
    pub speed_knots: Option<f64>,
    pub course_deg: Option<f64>,
}
impl<'a> GpsFix<'a> {
    /// Empty fields (no satellite fix) return None, not coordinates at zero.
    pub fn parse(line: &'a str) -> Result<Option<Self>, Error> {
        let mut fields = line
            .trim()
            .strip_prefix("+CGPSINFO:")
            .ok_or(Error::InvalidArgument)?
            .trim()
            .split(',');
        let mut f = [""; 9];
        for field in &mut f {
            *field = fields.next().ok_or(Error::InvalidArgument)?.trim();
        }
        if fields.next().is_some() {
            return Err(Error::InvalidArgument);
        }
        if f.iter().all(|s| s.is_empty()) {
            return Ok(None);
        }
        fn number(s: &str) -> Result<f64, Error> {
            let n = s.parse::<f64>().map_err(|_| Error::InvalidArgument)?;
            if n.is_finite() {
                Ok(n)
            } else {
                Err(Error::InvalidArgument)
            }
        }
        fn coordinate(s: &str, direction: &str, latitude: bool) -> Result<f64, Error> {
            let raw = number(s)?;
            if raw < 0.0 || raw > 18000.0 {
                return Err(Error::InvalidArgument);
            }
            let degrees = (raw / 100.0) as u32;
            let minutes = raw - f64::from(degrees) * 100.0;
            let value = f64::from(degrees) + minutes / 60.0;
            if minutes >= 60.0 || value > if latitude { 90.0 } else { 180.0 } {
                return Err(Error::InvalidArgument);
            }
            let sign = match (latitude, direction) {
                (true, "N") | (false, "E") => 1.0,
                (true, "S") | (false, "W") => -1.0,
                _ => return Err(Error::InvalidArgument),
            };
            Ok(value * sign)
        }
        fn optional(s: &str) -> Result<Option<f64>, Error> {
            if s.is_empty() {
                Ok(None)
            } else {
                number(s).map(Some)
            }
        }
        if f[4].len() != 6 || !f[4].bytes().all(|b| b.is_ascii_digit()) || f[5].is_empty() {
            return Err(Error::InvalidArgument);
        }
        number(f[5])?;
        Ok(Some(Self {
            latitude: coordinate(f[0], f[1], true)?,
            longitude: coordinate(f[2], f[3], false)?,
            date: f[4],
            utc_time: f[5],
            altitude_m: optional(f[6])?,
            speed_knots: optional(f[7])?,
            course_deg: optional(f[8])?,
        }))
    }
}
