use chrono::{DateTime, offset::TimeZone, Utc, Duration};
use std::fmt::Write;

pub struct TimestampKind {
    regex: regex::Regex,
    year: String,
    month: String,
    day: String,
    func: Box<dyn for<'a> Fn(&TimestampKind, &mut String, regex::Captures<'a>) -> chrono::format::ParseResult<DateTime<Utc>>>,
}

impl TimestampKind {
    pub fn new<F>(regex: &str, f: F) -> Self
    where
        F: 'static + for<'a> Fn(&TimestampKind, &mut String, regex::Captures<'a>) -> chrono::format::ParseResult<DateTime<Utc>>
    {
        let now = chrono::Utc::now();
        let year = now.format("%Y").to_string();
        let month = now.format("%m").to_string();
        let day = now.format("%d").to_string();

        TimestampKind {
            year, month, day,
            regex: regex::Regex::new(regex).unwrap(),
            func: Box::new(f),
        }
    }

    pub fn parse(&self, input: &str, temp_space: &mut String) -> Option<DateTime<Utc>> {
        temp_space.clear();
        match (&*self.func)(self, temp_space, self.regex.captures(input)?) {
            Ok(ts) => Some(ts),
            Err(_) => None
        }
    }
}

// Regex syntax at: https://docs.rs/regex/1.3.1/regex/#syntax
// Time format at: https://docs.rs/chrono/0.4.7/chrono/format/strftime/index.html

pub fn get_timestamp_kinds() -> Vec<TimestampKind> {
    vec![
        // Apr 6 17:13:40
        TimestampKind::new(r"^(\w{3} +\d+ +\d+:\d+:\d+)", |tk, s, caps| {
            let _ = write!(s, "{} {}", tk.year, caps.get(1).unwrap().as_str());
            Utc.datetime_from_str(s, "%Y %b %d %H:%M:%S")
        }),

        // 2018-12-15T02:11:06+0200
        TimestampKind::new(r"^(\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\+\d{4})", |_tk, _, caps| {
            DateTime::parse_from_str(caps.get(1).unwrap().as_str(), "%Y-%m-%dT%H:%M:%S%:z")
                .map(|x|From::from(x))
        }),

        // 2018-12-15T02:11:06.123456+02:00
        // 2019-10-09T10:58:45,929228489+03:00
        TimestampKind::new(r"^(\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2})[.,](\d{6})\d*(\+\d{2}):(\d{2})", |_tk, s, caps| {
            let _ = write!(s, "{}.{}{}{}", caps.get(1).unwrap().as_str(), caps.get(2).unwrap().as_str(),
                caps.get(3).unwrap().as_str(), caps.get(4).unwrap().as_str());
            DateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S%.f%z").map(|x|From::from(x))
        }),

        // Same as above, but with milliseconds
        TimestampKind::new(r"^(?:\d+\s+|\[|^)(\d{4}[/\-]\d{2}[/\-]\d{2}[ T]\d{2}:\d{2}:\d{2})(?:[.,](\d{3}))?", |_tk, _, caps| {
            let milliseconds = caps.get(2).map(|x|x.as_str().parse().unwrap()).unwrap_or(0);
            Utc.datetime_from_str(caps.get(1).unwrap().as_str(), "%Y-%m-%d %H:%M:%S")
                .map(|x| x + Duration::milliseconds(milliseconds))
        }),

        // 2018-04-06 17:13:40
        // [2018-04-06 17:13:40.955356
        // 1234 2018/04/06 17:13:40.955356
        // 1234 2018/04/06 17:13:40,955356
        // ...
        TimestampKind::new(r"^(?:\d+\s+|\[|^)(\d{4}[/\-]\d{2}[/\-]\d{2}[ T]\d{2}:\d{2}:\d{2})(?:[.,](\d{6}))?", |_tk, _, caps| {
            let microseconds = caps.get(2).map(|x|x.as_str().parse().unwrap()).unwrap_or(0);
            Utc.datetime_from_str(caps.get(1).unwrap().as_str(), "%Y-%m-%d %H:%M:%S")
                .map(|x| x + Duration::microseconds(microseconds))
        }),
   
        // for strace logs
        // 01:21:27
        // 01:21:27.554223
        TimestampKind::new(r"\b(\d{2}:\d{2}:\d{2})(?:\.(\d{6}))?", |tk, s, caps| {
            let _ = write!(s, "{}.{}.{} {}", tk.year, tk.month, tk.day, caps.get(1).unwrap().as_str());
            Utc.datetime_from_str(s, "%Y.%m.%d %H:%M:%S")
                .map(|x| x + Duration::microseconds(caps.get(2).map(|x|x.as_str().parse().unwrap()).unwrap_or(0)))
        }),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn main() {
        let samples = &[
            "01:21:27",
            "Apr 6 17:13:40",
            "2018-12-15T02:11:06+0200",
            "2018-12-15T02:11:06.123456+02:00",
            "2019-10-09T10:58:45,929228489+03:00",
            "2018-04-06 17:13:40,955",
            "2018-04-23 04:48:11,811|",
            "2018-04-06 17:13:40",
            "1564 2020-01-15 14:54:14.558",
            "2018-04-06 17:13:40.955356",
            "[2018/04/06 17:13:40",
            "[2018/04/06 17:13:40.955356",
            "16255 15:08:52.554223",
        ];

        for sample in samples {
            let string = format!("{}{}", sample, " log line");
            let mut matches = 0;

            for (index, kind) in get_timestamp_kinds().iter().enumerate() {
                match kind.parse(&string, &mut String::new()) {
                    None => {},
                    Some(value) => {
                        println!("kind {} matched sample {:?}, got value {:?}", index, sample, value);
                        matches += 1;
                    }
                }
            }

            if matches == 0 {
                panic!("sample {:?} did not match any timestamp kind", sample);
            }
        }
    }
}
