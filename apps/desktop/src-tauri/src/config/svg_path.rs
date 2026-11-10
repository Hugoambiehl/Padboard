//! Syntax check of SVG path data (`d` attribute), so that only well-formed
//! paths reach the tablet's path parser.

/// `true` if `d` is valid SVG path data: starts with a moveto, every command
/// has a whole number of argument groups, arc flags are 0 or 1.
#[must_use]
pub fn is_valid(d: &str) -> bool {
    let mut p = Parser {
        bytes: d.as_bytes(),
        pos: 0,
    };
    p.skip_separators();
    if p.pos == p.bytes.len() {
        return false;
    }
    let mut first = true;
    while p.pos < p.bytes.len() {
        let Some(command) = p.command() else {
            return false;
        };
        if first && !matches!(command, b'M' | b'm') {
            return false;
        }
        first = false;
        let ok = match command.to_ascii_uppercase() {
            b'Z' => true,
            b'M' | b'L' | b'T' => p.groups(&[Arg::Number; 2]),
            b'H' | b'V' => p.groups(&[Arg::Number]),
            b'S' | b'Q' => p.groups(&[Arg::Number; 4]),
            b'C' => p.groups(&[Arg::Number; 6]),
            b'A' => p.groups(&[
                Arg::Number,
                Arg::Number,
                Arg::Number,
                Arg::Flag,
                Arg::Flag,
                Arg::Number,
                Arg::Number,
            ]),
            _ => false,
        };
        if !ok {
            return false;
        }
    }
    true
}

#[derive(Clone, Copy)]
enum Arg {
    Number,
    Flag,
}

struct Parser<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl Parser<'_> {
    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.pos).copied()
    }

    fn skip_separators(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t' | b'\n' | b'\r' | b',')) {
            self.pos += 1;
        }
    }

    fn command(&mut self) -> Option<u8> {
        let c = self.peek()?;
        if !c.is_ascii_alphabetic() {
            return None;
        }
        self.pos += 1;
        self.skip_separators();
        Some(c)
    }

    /// One or more argument groups (until the next command or the end).
    fn groups(&mut self, shape: &[Arg]) -> bool {
        let mut count = 0;
        loop {
            match self.peek() {
                None => break,
                Some(c) if c.is_ascii_alphabetic() && c != b'e' && c != b'E' => break,
                _ => {}
            }
            for arg in shape {
                let ok = match arg {
                    Arg::Number => self.number(),
                    Arg::Flag => self.flag(),
                };
                if !ok {
                    return false;
                }
                self.skip_separators();
            }
            count += 1;
        }
        count > 0
    }

    fn flag(&mut self) -> bool {
        if matches!(self.peek(), Some(b'0' | b'1')) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn digits(&mut self) -> usize {
        let start = self.pos;
        while self.peek().is_some_and(|c| c.is_ascii_digit()) {
            self.pos += 1;
        }
        self.pos - start
    }

    fn number(&mut self) -> bool {
        if matches!(self.peek(), Some(b'+' | b'-')) {
            self.pos += 1;
        }
        let mut mantissa = self.digits();
        if self.peek() == Some(b'.') {
            self.pos += 1;
            mantissa += self.digits();
        }
        if mantissa == 0 {
            return false;
        }
        if matches!(self.peek(), Some(b'e' | b'E')) {
            self.pos += 1;
            if matches!(self.peek(), Some(b'+' | b'-')) {
                self.pos += 1;
            }
            if self.digits() == 0 {
                return false;
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::is_valid;

    #[test]
    fn accepts_lucide_paths() {
        for d in [
            "M12 19h8",
            "m4 17 6-6-6-6",
            "M11 4.702a.705.705 0 0 0-1.203-.498L6.413 7.587A1.4 1.4 0 0 1 5.416 8H3",
            "M9 13a3 3 0 1 0 6 0a3 3 0 1 0 -6 0z",
            "M4 3H20A2 2 0 0 1 22 5V15z",
            "M1e2,3E-1 L4 5",
            "M5 5 a1 1 0 01 2 2",
        ] {
            assert!(is_valid(d), "{d}");
        }
    }

    #[test]
    fn rejects_malformed_paths() {
        for d in [
            "",
            "   ",
            "L1 2",
            "M1",
            "M1 2 3",
            "M1 2 X3 4",
            "M1 2 L",
            "M1 2 A1 1 0 2 0 3 3",
            "M1 2 <script>",
            "M. 2",
            "M1e 2",
        ] {
            assert!(!is_valid(d), "{d:?}");
        }
    }
}
