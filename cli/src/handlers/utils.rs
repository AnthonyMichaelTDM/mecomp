use core::fmt;
use std::io::{self, BufRead, IsTerminal, Stdin};

use mecomp_prost::RecordId;

pub fn parse_from_lines<Lines, Out>(lines: Lines) -> Vec<Out>
where
    Lines: Iterator<Item = String>,
    Out: std::str::FromStr,
{
    lines.fold(Vec::new(), |mut acc, line| {
        if let Ok(thing) = line.parse() {
            acc.push(thing);
        }
        acc
    })
}

/// Check if we should read from stdin
/// Returns true if:
/// - stdin is not a terminal (data is being piped), OR
/// - the optional parameter is None (user didn't provide an argument)
/// This allows both explicit piping and implicit piping when the argument is omitted
pub fn should_read_from_stdin<T>(stdin: &Stdin, optional_param: &Option<T>) -> bool {
    !stdin.is_terminal() || optional_param.is_none()
}

/// Read RecordIds from stdin, filtering and handling errors
pub fn read_record_ids_from_stdin<W: fmt::Write>(
    stdin: Stdin,
    stderr: &mut W,
) -> Vec<RecordId> {
    parse_from_lines(stdin.lock().lines().filter_map(|l| match l {
        Ok(line) => Some(line),
        Err(e) => {
            writeln!(stderr, "Error reading from stdin: {e}").ok();
            None
        }
    }))
}

pub struct WriteAdapter<W>(pub W);

impl<W> fmt::Write for WriteAdapter<W>
where
    W: io::Write,
{
    fn write_str(&mut self, s: &str) -> Result<(), fmt::Error> {
        self.0.write_all(s.as_bytes()).map_err(|_| fmt::Error)
    }

    fn write_fmt(&mut self, args: fmt::Arguments<'_>) -> Result<(), fmt::Error> {
        self.0.write_fmt(args).map_err(|_| fmt::Error)
    }
}
