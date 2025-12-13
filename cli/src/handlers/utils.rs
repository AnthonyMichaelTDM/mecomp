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
