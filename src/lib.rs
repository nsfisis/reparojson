use std::io::{BufReader, Read, Write};
use std::iter::Peekable;

pub type RepairResult = Result<RepairOk, RepairErr>;

#[derive(Debug)]
pub enum RepairOk {
    Valid,
    Repaired,
}

#[derive(Debug)]
pub enum RepairErr {
    Invalid(SyntaxError),
    IoErr(std::io::Error),
}

impl From<std::io::Error> for RepairErr {
    fn from(value: std::io::Error) -> Self {
        Self::IoErr(value)
    }
}

impl From<SyntaxError> for RepairErr {
    fn from(value: SyntaxError) -> Self {
        Self::Invalid(value)
    }
}

#[derive(Debug)]
pub enum SyntaxError {
    UnexpectedEof,
    InvalidValue,
    TrailingData,
}

impl std::fmt::Display for SyntaxError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::UnexpectedEof => write!(f, "unexpected end of file"),
            Self::InvalidValue => write!(f, "invalid value"),
            Self::TrailingData => write!(f, "unexpected data at the end"),
        }
    }
}

pub fn repair(r: impl Read, mut w: impl Write) -> RepairResult {
    let mut r = BufReader::new(r).bytes().peekable();
    let mut p = Parser::new(&mut r, &mut w);
    match p.walk_json() {
        Ok(_) => Ok(if p.repaired() {
            RepairOk::Repaired
        } else {
            RepairOk::Valid
        }),
        Err(err) => Err(err),
    }
}

struct Parser<'input, 'output, I: ByteStream, W: Write> {
    input: &'input mut I,
    output: &'output mut W,
    repaired: bool,
}

type ParserResult = Result<(), RepairErr>;

trait ByteStream {
    fn next(&mut self) -> Result<std::io::Result<u8>, SyntaxError> {
        match self.try_next() {
            Some(ret) => Ok(ret),
            None => Err(SyntaxError::UnexpectedEof),
        }
    }

    fn peek(&mut self) -> Result<std::io::Result<u8>, SyntaxError> {
        match self.try_peek() {
            Some(ret) => Ok(ret),
            None => Err(SyntaxError::UnexpectedEof),
        }
    }

    fn skip(&mut self) {
        let res = self.try_next();
        assert!(matches!(res, Some(Ok(_))));
    }

    fn eof(&mut self) -> bool {
        self.try_next().is_none()
    }

    fn try_next(&mut self) -> Option<std::io::Result<u8>>;
    fn try_peek(&mut self) -> Option<std::io::Result<u8>>;
}

impl<I: Iterator<Item = std::io::Result<u8>>> ByteStream for Peekable<I> {
    fn try_next(&mut self) -> Option<std::io::Result<u8>> {
        Iterator::next(self)
    }

    fn try_peek(&mut self) -> Option<std::io::Result<u8>> {
        match Peekable::peek(self) {
            Some(Ok(c)) => Some(Ok(*c)),
            Some(Err(_)) => Some(Err(Iterator::next(self)
                .expect("next() returns some value because peek() returned some value.")
                .expect_err("next() returns some error because peek() returned some error."))),
            None => None,
        }
    }
}

impl<'input, 'output, I: ByteStream, W: Write> Parser<'input, 'output, I, W> {
    fn new(input: &'input mut I, output: &'output mut W) -> Self {
        Self {
            input,
            output,
            repaired: false,
        }
    }

    fn repaired(&self) -> bool {
        self.repaired
    }

    fn walk_json(&mut self) -> ParserResult {
        self.walk_element()?;
        if self.input.eof() {
            Ok(())
        } else {
            Err(SyntaxError::TrailingData.into())
        }
    }

    fn walk_value(&mut self) -> ParserResult {
        let c = self.input.peek()??;

        match c {
            b'n' => {
                self.input.skip(); // => n
                self.output.write_all(b"n")?;
                self.walk_char_of(b'u')?;
                self.walk_char_of(b'l')?;
                self.walk_char_of(b'l')?;
                Ok(())
            }
            b't' => {
                self.input.skip(); // => t
                self.output.write_all(b"t")?;
                self.walk_char_of(b'r')?;
                self.walk_char_of(b'u')?;
                self.walk_char_of(b'e')?;
                Ok(())
            }
            b'f' => {
                self.input.skip(); // => f
                self.output.write_all(b"f")?;
                self.walk_char_of(b'a')?;
                self.walk_char_of(b'l')?;
                self.walk_char_of(b's')?;
                self.walk_char_of(b'e')?;
                Ok(())
            }
            b'{' => self.walk_object(),
            b'[' => self.walk_array(),
            b'"' => self.walk_string(),
            b'-' => self.walk_number(),
            c if c.is_ascii_digit() => self.walk_number(),
            _ => Err(SyntaxError::InvalidValue.into()),
        }
    }

    fn walk_object(&mut self) -> ParserResult {
        self.output.write_all(b"{")?;
        self.input.skip(); // => {

        self.walk_ws()?;

        // members_opt
        let first = self.input.peek()??;
        if first == b'"' {
            self.walk_members()?;
        }

        // trailing_comma_opt
        let maybe_comma = self.input.peek()??;
        if maybe_comma == b',' {
            self.repaired = true;
            self.input.skip();
            self.walk_ws()?;
        }

        self.walk_char_of(b'}')
    }

    fn walk_members(&mut self) -> ParserResult {
        loop {
            self.walk_member()?;

            let mut ws = Vec::with_capacity(1024);
            self.walk_ws_with_buf(&mut ws)?;

            let next = self.input.peek()??;
            match next {
                b'}' => {
                    self.output.write_all(&ws)?;
                    return Ok(());
                }
                b',' => {
                    self.output.write_all(&ws)?;
                    // Re-use the memory buffer to avoid another allocation.
                    ws.clear();

                    self.input.skip();

                    self.walk_ws_with_buf(&mut ws)?;

                    let c = self.input.peek()??;
                    match c {
                        b'}' => {
                            self.repaired = true;
                            self.output.write_all(&ws)?;
                            return Ok(());
                        }
                        _ => {
                            self.output.write_all(b",")?;
                            self.output.write_all(&ws)?;
                        }
                    }
                }
                _ => {
                    self.repaired = true;
                    self.output.write_all(b",")?;
                    self.output.write_all(&ws)?;
                }
            }
        }
    }

    fn walk_member(&mut self) -> ParserResult {
        self.walk_string()?;
        self.walk_ws()?;
        self.walk_char_of(b':')?;
        self.walk_ws()?;
        self.walk_value()
    }

    fn walk_array(&mut self) -> ParserResult {
        self.output.write_all(b"[")?;
        self.input.skip(); // => [

        self.walk_ws()?;

        // elements_opt
        let first = self.input.peek()??;
        if first != b',' && first != b']' {
            self.walk_elements()?;
        }

        // trailing_comma_opt
        let maybe_comma = self.input.peek()??;
        if maybe_comma == b',' {
            self.repaired = true;
            self.input.skip();
            self.walk_ws()?;
        }

        self.walk_char_of(b']')
    }

    fn walk_elements(&mut self) -> ParserResult {
        loop {
            self.walk_value()?;

            let mut ws = Vec::with_capacity(1024);
            self.walk_ws_with_buf(&mut ws)?;

            let next = self.input.peek()??;
            match next {
                b']' => {
                    self.output.write_all(&ws)?;
                    return Ok(());
                }
                b',' => {
                    self.output.write_all(&ws)?;
                    // Re-use the memory buffer to avoid another allocation.
                    ws.clear();

                    self.input.skip();

                    self.walk_ws_with_buf(&mut ws)?;

                    let c = self.input.peek()??;
                    match c {
                        b']' => {
                            self.repaired = true;
                            self.output.write_all(&ws)?;
                            return Ok(());
                        }
                        _ => {
                            self.output.write_all(b",")?;
                            self.output.write_all(&ws)?;
                        }
                    }
                }
                _ => {
                    self.repaired = true;
                    self.output.write_all(b",")?;
                    self.output.write_all(&ws)?;
                }
            }
        }
    }

    fn walk_element(&mut self) -> ParserResult {
        self.walk_ws()?;
        self.walk_value()?;
        self.walk_ws()
    }

    fn walk_string(&mut self) -> ParserResult {
        self.output.write_all(b"\"")?;
        self.input.skip(); // => "
        loop {
            match self.input.next()?? {
                b'"' => break,
                b'\\' => {
                    self.walk_escape()?;
                }
                c => {
                    self.output.write_all(&[c])?;
                }
            }
        }
        self.output.write_all(b"\"")?;
        Ok(())
    }

    fn walk_escape(&mut self) -> ParserResult {
        let c = self.input.next()??;
        match c {
            b'"' | b'\\' | b'/' | b'b' | b'f' | b'n' | b'r' | b't' => {
                self.output.write_all(&[b'\\', c])?;
            }
            b'u' => {
                let u1 = self.input.next()??;
                let u2 = self.input.next()??;
                let u3 = self.input.next()??;
                let u4 = self.input.next()??;
                if !u1.is_ascii_hexdigit()
                    || !u2.is_ascii_hexdigit()
                    || !u3.is_ascii_hexdigit()
                    || !u4.is_ascii_hexdigit()
                {
                    return Err(SyntaxError::InvalidValue.into());
                }
                self.output.write_all(&[b'\\', b'u', u1, u2, u3, u4])?;
            }
            _ => return Err(SyntaxError::InvalidValue.into()),
        }
        Ok(())
    }

    fn walk_number(&mut self) -> ParserResult {
        self.walk_integer()?;
        self.walk_fraction()?;
        self.walk_exponent()
    }

    fn walk_integer(&mut self) -> ParserResult {
        let first = self.input.next()??;
        match first {
            b'-' => {
                self.output.write_all(b"-")?;
                return self.walk_integer();
            }
            b'0' => {
                self.output.write_all(b"0")?;
                return Ok(());
            }
            b'1' | b'2' | b'3' | b'4' | b'5' | b'6' | b'7' | b'8' | b'9' => {
                self.output.write_all(&[first])?;
                loop {
                    let Some(c) = self.input.try_peek() else {
                        return Ok(());
                    };
                    let c = c?;
                    if c.is_ascii_digit() {
                        self.output.write_all(&[c])?;
                        self.input.skip();
                    } else {
                        break;
                    }
                }
            }
            _ => return Err(SyntaxError::InvalidValue.into()),
        }
        Ok(())
    }

    fn walk_digits(&mut self) -> ParserResult {
        let mut has_digit = false;
        while let Some(c) = self.input.try_peek() {
            let c = c?;
            if c.is_ascii_digit() {
                self.output.write_all(&[c])?;
                self.input.skip();
                has_digit = true;
            } else {
                break;
            }
        }
        if has_digit {
            Ok(())
        } else {
            Err(SyntaxError::InvalidValue.into())
        }
    }

    fn walk_fraction(&mut self) -> ParserResult {
        let Some(first) = self.input.try_peek() else {
            return Ok(());
        };
        let first = first?;
        if first != b'.' {
            return Ok(());
        }
        self.output.write_all(b".")?;
        self.input.skip();
        self.walk_digits()
    }

    fn walk_exponent(&mut self) -> ParserResult {
        let Some(first) = self.input.try_peek() else {
            return Ok(());
        };
        let first = first?;
        if first != b'e' && first != b'E' {
            return Ok(());
        }
        self.output.write_all(&[first])?;
        self.input.skip();
        self.walk_sign()?;
        self.walk_digits()
    }

    fn walk_sign(&mut self) -> ParserResult {
        let c = self.input.peek()??;
        if c == b'+' || c == b'-' {
            self.output.write_all(&[c])?;
            self.input.skip();
        }
        Ok(())
    }

    fn walk_ws(&mut self) -> ParserResult {
        Self::do_walk_ws(self.input, self.output)
    }

    fn walk_ws_with_buf(&mut self, buf: &mut Vec<u8>) -> ParserResult {
        Self::do_walk_ws(self.input, buf)
    }

    fn do_walk_ws<Output: Write>(input: &mut I, output: &mut Output) -> ParserResult {
        loop {
            let Some(c) = input.try_peek() else {
                return Ok(());
            };
            let c = c?;
            match c {
                0x09 | 0x0A | 0x0D | 0x20 => {
                    output.write_all(&[c])?;
                    input.skip();
                }
                _ => return Ok(()),
            }
        }
    }

    fn walk_char_of(&mut self, expected: u8) -> ParserResult {
        let c = self.input.next()??;
        if c != expected {
            return Err(SyntaxError::InvalidValue.into());
        }
        self.output.write_all(&[c])?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    fn repair(input: &str) -> (super::RepairResult, String) {
        let mut output = Vec::new();
        let result = super::repair(input.as_bytes(), &mut output);
        (result, String::from_utf8(output).unwrap())
    }

    #[test]
    fn test_repair_invalid() {
        assert!(repair(r#"foo"#).0.is_err());
        assert!(repair(r#"{{}"#).0.is_err());
        assert!(repair(r#"[]]"#).0.is_err());
        assert!(repair(r#"[,,]"#).0.is_err());
        assert!(repair(r#"[,,,]"#).0.is_err());
        assert!(repair(r#"{,,}"#).0.is_err());
        assert!(repair(r#"{,,,}"#).0.is_err());
    }

    #[test]
    fn test_repair_valid() {
        {
            let s = r#"null"#;
            let (res, out) = repair(s);
            assert!(res.is_ok());
            assert_eq!(s, out);
        }
        {
            let s = r#" true"#;
            let (res, out) = repair(s);
            assert!(res.is_ok());
            assert_eq!(s, out);
        }
        {
            let s = r#" false "#;
            let (res, out) = repair(s);
            assert!(res.is_ok());
            assert_eq!(s, out);
        }
        {
            let s = r#" 123.0e-1 "#;
            let (res, out) = repair(s);
            assert!(res.is_ok());
            assert_eq!(s, out);
        }
        {
            let s = r#""foo\"bar\"""#;
            let (res, out) = repair(s);
            assert!(res.is_ok());
            assert_eq!(s, out);
        }
    }

    #[test]
    fn test_repair_repaired() {
        {
            let s = r#"[  , ]"#;
            let (res, out) = repair(s);
            assert!(matches!(res, Ok(super::RepairOk::Repaired)));
            assert_eq!("[   ]", out);
        }
        {
            let s = r#"[   1 ,  ]"#;
            let (res, out) = repair(s);
            assert!(matches!(res, Ok(super::RepairOk::Repaired)));
            assert_eq!("[   1   ]", out);
        }
        {
            let s = r#"[1   2  ]"#;
            let (res, out) = repair(s);
            assert!(matches!(res, Ok(super::RepairOk::Repaired)));
            assert_eq!("[1,   2  ]", out);
        }
        {
            let s = r#"[1   2  ,]"#;
            let (res, out) = repair(s);
            assert!(matches!(res, Ok(super::RepairOk::Repaired)));
            assert_eq!("[1,   2  ]", out);
        }
        {
            let s = r#"{  , }"#;
            let (res, out) = repair(s);
            assert!(matches!(res, Ok(super::RepairOk::Repaired)));
            assert_eq!(r#"{   }"#, out);
        }
        {
            let s = r#"{   "a":1 ,  }"#;
            let (res, out) = repair(s);
            assert!(matches!(res, Ok(super::RepairOk::Repaired)));
            assert_eq!(r#"{   "a":1   }"#, out);
        }
        {
            let s = r#"{"a":1   "b":2  }"#;
            let (res, out) = repair(s);
            assert!(matches!(res, Ok(super::RepairOk::Repaired)));
            assert_eq!(r#"{"a":1,   "b":2  }"#, out);
        }
        {
            let s = r#"{"a":1   "b":2  ,}"#;
            let (res, out) = repair(s);
            assert!(matches!(res, Ok(super::RepairOk::Repaired)));
            assert_eq!(r#"{"a":1,   "b":2  }"#, out);
        }
    }
}
