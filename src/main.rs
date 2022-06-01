mod timestamp_kinds;
mod timestamp_stream;

use timestamp_kinds::get_timestamp_kinds;
use timestamp_stream::{PossibleTimestampKinds, TimestampedStream};
use structopt::StructOpt;
use itertools::Itertools;
use chrono::{DateTime, Utc};

use std::path::PathBuf;
use std::io::Write;
use std::fs::File;
use std::rc::Rc;

#[derive(Debug)]
struct PrefixedFile(String, PathBuf);

impl std::str::FromStr for PrefixedFile {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s.find('=') {
            None => Self("".to_owned(), PathBuf::from(s)),
            Some(index) => Self(s[..index].to_owned(), PathBuf::from(&s[index + 1..])),
        })
    }
}

#[derive(Debug, StructOpt)]
#[structopt(name = "ziplog", about = "ZipLog - merge logs by timestamps")]
struct Opt {
    /// The default prefix to prepend to timestamped lines
    #[structopt(short = "p", long = "prefix", default_value = "> ")]
    prefix: String,

    /// Show interval by seconds (s), or milliseconds (ms)
    #[structopt(short = "i", long = "interval")]
    interval: Option<String>,

    /// Log files; Use "-" for STDIN
    #[structopt(name = "FILE")]
    logs: Vec<PathBuf>,

    /// Prefixed log files, using a different prefix for each timestamped file
    #[structopt(short = "f", long = "--prefixed-file")]
    prefixed_logs: Vec<PrefixedFile>,
}

type Stream = Box<dyn Iterator<Item=(Option<DateTime<Utc>>, String)>>;

fn add_stream(path: &PathBuf, prefix: &String, timestamp_kinds: &PossibleTimestampKinds,
    stdin_found: &mut bool, streams: &mut Vec<Stream>) -> std::io::Result<()>
{
    let stream : Box<dyn Iterator<Item=(Option<DateTime<Utc>>, String)>> = if path == &PathBuf::from("-") {
        if *stdin_found {
            return Ok(());
        }

        *stdin_found = true;
        Box::new(TimestampedStream::new(std::io::stdin(),
            prefix.clone(), timestamp_kinds.clone()))
    } else {
        let file = match File::open(&path) {
            Err(err) => {
                eprintln!("Error opening file: {:?}", path);
                return Err(err);
            }
            Ok(v) => v,
        };
        Box::new(TimestampedStream::new(file,
            prefix.clone(), timestamp_kinds.clone()))
    };

    streams.push(stream);
    Ok(())
}

fn main() -> std::io::Result<()> {
    // Command line parsing

    let opt = Opt::from_args();

    enum IntervalType {
        Seconds,
        Milliseconds,
    }

    let interval = match opt.interval.as_ref().map(|x| x.as_str()) {
        Some("s") => Some(IntervalType::Seconds),
        Some("ms") => Some(IntervalType::Milliseconds),
        None => None,
        _ => {
            return Err(std::io::Error::new(std::io::ErrorKind::InvalidInput,
                    "invalid interval specifier"));
        },
    };

    // Load up the files

    let timestamp_kinds : PossibleTimestampKinds =
        Rc::new(get_timestamp_kinds().into_iter().map(|x| Rc::new(x)).collect());
    let mut streams = vec![];
    let mut stdin_found = false;

    for path in opt.logs {
        add_stream(&path, &opt.prefix, &timestamp_kinds, &mut stdin_found, &mut streams)?;
    }

    for prefixed_log in opt.prefixed_logs {
        add_stream(&prefixed_log.1, &prefixed_log.0, &timestamp_kinds, &mut stdin_found, &mut streams)?;
    }

    // Iterate-merge all the lines

    let mut stdout = grep_cli::stdout(termcolor::ColorChoice::Auto);

    let mut prev_ts : Option<DateTime<Utc>> = None;
    for item in streams.into_iter().kmerge() {
        match &interval {
            None => println!("{}", item.1),
            Some(interval_kind) => {
                if let (Some(prev), Some(cur)) = (prev_ts, item.0) {
                    let diff = cur - prev;
                    let diff = match interval_kind {
                        IntervalType::Seconds => diff.num_seconds(),
                        IntervalType::Milliseconds => diff.num_milliseconds(),
                    };
                    let _ = writeln!(stdout, "{:7}{}", diff, item.1);
                } else {
                    let _ = writeln!(stdout, "{:7}{}", "", item.1);
                }

                if item.0.is_some() {
                    prev_ts = item.0;
                }
            }
        }
    }

    Ok(())
}
