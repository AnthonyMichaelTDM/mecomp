use core::fmt;
use std::io::{self, BufRead, IsTerminal};

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

/// Extends the given list of items with items parsed from stdin iff stdin is not a terminal.
///
/// invalid lines are skipped with an error message to stderr.
///
/// # Errors
///
/// Errors if there is an error reading from stdin, or if the final list of items would be empty.
pub fn extend_from_stdin<Out: std::str::FromStr>(
    mut items: Vec<Out>,
    stdin: &impl StdIn,
    stderr: &mut impl std::fmt::Write,
) -> Result<Vec<Out>, io::Error> {
    // are we in a pipe?
    if stdin.is_terminal() && items.is_empty() {
        Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "no input provided",
        ))
    } else if !stdin.is_terminal() {
        let from_pipe: Vec<Out> = parse_from_lines(stdin.lines().filter_map(|l| match l {
            Ok(line) => Some(line),
            Err(e) => {
                writeln!(stderr, "Error reading from stdin: {e}").ok();
                None
            }
        }));
        items.extend(from_pipe);
        if items.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "no input provided",
            ));
        }
        Ok(items)
    } else {
        Ok(items)
    }
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

pub trait StdIn: Send + Sync {
    fn is_terminal(&self) -> bool;
    fn lines(&self) -> impl Iterator<Item = io::Result<String>>;
}

impl StdIn for io::Stdin {
    fn is_terminal(&self) -> bool {
        self.lock().is_terminal()
    }
    fn lines(&self) -> impl Iterator<Item = io::Result<String>> {
        io::BufReader::new(self.lock()).lines()
    }
}
