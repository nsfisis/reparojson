use std::io::{Read, Write};
use std::iter::Peekable;

pub type RepairResult = Result<RepairOk, RepairErr>;

pub enum RepairOk {
    Valid,
    Repaired,
}

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
    let mut p = Parser::new();
    match p.walk_json(&mut r.bytes().peekable(), &mut w) {
        Ok(_) => Ok(if p.repaired() {
            RepairOk::Repaired
        } else {
            RepairOk::Valid
        }),
        Err(err) => Err(err),
    }
}

struct Parser {
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

impl Parser {
    fn new() -> Self {
        Self { repaired: false }
    }

    fn repaired(&self) -> bool {
        self.repaired
    }

    fn walk_json<I: ByteStream, W: Write>(&mut self, input: &mut I, w: &mut W) -> ParserResult {
        self.walk_element(input, w)?;
        if input.eof() {
            Ok(())
        } else {
            Err(SyntaxError::TrailingData.into())
        }
    }

    fn walk_value<I: ByteStream, W: Write>(&mut self, input: &mut I, w: &mut W) -> ParserResult {
        let c = input.peek()??;

        match c {
            b'n' => {
                input.skip(); // => n
                let c2 = input.next()??; // u?
                let c3 = input.next()??; // l?
                let c4 = input.next()??; // l?
                if !matches!((c2, c3, c4), (b'u', b'l', b'l')) {
                    return Err(SyntaxError::InvalidValue.into());
                }
                w.write_all(b"null")?;
                Ok(())
            }
            b't' => {
                input.skip(); // => t
                let c2 = input.next()??; // r?
                let c3 = input.next()??; // u?
                let c4 = input.next()??; // e?
                if !matches!((c2, c3, c4), (b'r', b'u', b'e')) {
                    return Err(SyntaxError::InvalidValue.into());
                }
                w.write_all(b"true")?;
                Ok(())
            }
            b'f' => {
                input.skip(); // => f
                let c2 = input.next()??; // a?
                let c3 = input.next()??; // l?
                let c4 = input.next()??; // s?
                let c5 = input.next()??; // e?
                if !matches!((c2, c3, c4, c5), (b'a', b'l', b's', b'e')) {
                    return Err(SyntaxError::InvalidValue.into());
                }
                w.write_all(b"false")?;
                Ok(())
            }
            b'{' => self.walk_object(input, w),
            b'[' => self.walk_array(input, w),
            b'"' => self.walk_string(input, w),
            b'-' => self.walk_number(input, w),
            c if c.is_ascii_digit() => self.walk_number(input, w),
            _ => Err(SyntaxError::InvalidValue.into()),
        }
    }

    fn walk_object<I: ByteStream, W: Write>(&mut self, input: &mut I, w: &mut W) -> ParserResult {
        w.write_all(b"{")?;
        input.skip(); // => {

        self.walk_ws(input, w)?;

        // members_opt
        let first = input.peek()??;
        if first == b'"' {
            self.walk_members(input, w)?;
        }

        // trailing_comma_opt
        let maybe_comma = input.peek()??;
        if maybe_comma == b',' {
            self.repaired = true;
            input.skip();
            self.walk_ws(input, w)?;
        }

        let last = input.next()??;
        if last != b'}' {
            return Err(SyntaxError::InvalidValue.into());
        }
        w.write_all(b"}")?;
        Ok(())
    }

    fn walk_members<I: ByteStream, W: Write>(&mut self, input: &mut I, w: &mut W) -> ParserResult {
        loop {
            self.walk_member(input, w)?;

            let mut ws = Vec::with_capacity(1024);
            self.walk_ws(input, &mut ws)?;

            let next = input.peek()??;
            match next {
                b'}' => {
                    w.write_all(&ws)?;
                    return Ok(());
                }
                b',' => {
                    w.write_all(&ws)?;
                    // Re-use the memory buffer to avoid another allocation.
                    ws.clear();

                    input.skip();

                    self.walk_ws(input, &mut ws)?;

                    let c = input.peek()??;
                    match c {
                        b'}' => {
                            self.repaired = true;
                            w.write_all(&ws)?;
                            return Ok(());
                        }
                        _ => {
                            w.write_all(b",")?;
                            w.write_all(&ws)?;
                        }
                    }
                }
                _ => {
                    self.repaired = true;
                    w.write_all(b",")?;
                    w.write_all(&ws)?;
                }
            }
        }
    }

    fn walk_member<I: ByteStream, W: Write>(&mut self, input: &mut I, w: &mut W) -> ParserResult {
        self.walk_string(input, w)?;
        self.walk_ws(input, w)?;
        let colon = input.next()??;
        if colon != b':' {
            return Err(SyntaxError::InvalidValue.into());
        }
        w.write_all(b":")?;
        self.walk_ws(input, w)?;
        self.walk_value(input, w)
    }

    fn walk_array<I: ByteStream, W: Write>(&mut self, input: &mut I, w: &mut W) -> ParserResult {
        w.write_all(b"[")?;
        input.skip(); // => [

        self.walk_ws(input, w)?;

        // elements_opt
        let first = input.peek()??;
        if first != b',' && first != b']' {
            self.walk_elements(input, w)?;
        }

        // trailing_comma_opt
        let maybe_comma = input.peek()??;
        if maybe_comma == b',' {
            self.repaired = true;
            input.skip();
            self.walk_ws(input, w)?;
        }

        let last = input.next()??;
        if last != b']' {
            return Err(SyntaxError::InvalidValue.into());
        }
        w.write_all(b"]")?;
        Ok(())
    }

    fn walk_elements<I: ByteStream, W: Write>(&mut self, input: &mut I, w: &mut W) -> ParserResult {
        loop {
            self.walk_value(input, w)?;

            let mut ws = Vec::with_capacity(1024);
            self.walk_ws(input, &mut ws)?;

            let next = input.peek()??;
            match next {
                b']' => {
                    w.write_all(&ws)?;
                    return Ok(());
                }
                b',' => {
                    w.write_all(&ws)?;
                    // Re-use the memory buffer to avoid another allocation.
                    ws.clear();

                    input.skip();

                    self.walk_ws(input, &mut ws)?;

                    let c = input.peek()??;
                    match c {
                        b']' => {
                            self.repaired = true;
                            w.write_all(&ws)?;
                            return Ok(());
                        }
                        _ => {
                            w.write_all(b",")?;
                            w.write_all(&ws)?;
                        }
                    }
                }
                _ => {
                    self.repaired = true;
                    w.write_all(b",")?;
                    w.write_all(&ws)?;
                }
            }
        }
    }

    fn walk_element<I: ByteStream, W: Write>(&mut self, input: &mut I, w: &mut W) -> ParserResult {
        self.walk_ws(input, w)?;
        self.walk_value(input, w)?;
        self.walk_ws(input, w)
    }

    fn walk_string<I: ByteStream, W: Write>(&mut self, input: &mut I, w: &mut W) -> ParserResult {
        w.write_all(b"\"")?;
        input.skip(); // => "
        loop {
            match input.next()?? {
                b'"' => break,
                b'\\' => {
                    self.walk_escape(input, w)?;
                }
                c => {
                    w.write_all(&[c])?;
                }
            }
        }
        w.write_all(b"\"")?;
        Ok(())
    }

    fn walk_escape<I: ByteStream, W: Write>(&mut self, input: &mut I, w: &mut W) -> ParserResult {
        let c = input.next()??;
        match c {
            b'"' | b'\\' | b'/' | b'b' | b'f' | b'n' | b'r' | b't' => {
                w.write_all(&[b'\\', c])?;
            }
            b'u' => {
                let u1 = input.next()??;
                let u2 = input.next()??;
                let u3 = input.next()??;
                let u4 = input.next()??;
                if !u1.is_ascii_hexdigit()
                    || !u2.is_ascii_hexdigit()
                    || !u3.is_ascii_hexdigit()
                    || !u4.is_ascii_hexdigit()
                {
                    return Err(SyntaxError::InvalidValue.into());
                }
                w.write_all(&[b'\\', u1, u2, u3, u4])?;
            }
            _ => return Err(SyntaxError::InvalidValue.into()),
        }
        Ok(())
    }

    fn walk_number<I: ByteStream, W: Write>(&mut self, input: &mut I, w: &mut W) -> ParserResult {
        self.walk_integer(input, w)?;
        self.walk_fraction(input, w)?;
        self.walk_exponent(input, w)
    }

    fn walk_integer<I: ByteStream, W: Write>(&mut self, input: &mut I, w: &mut W) -> ParserResult {
        let first = input.next()??;
        match first {
            b'-' => {
                w.write_all(b"-")?;
                return self.walk_integer(input, w);
            }
            b'0' => {
                w.write_all(b"0")?;
                return Ok(());
            }
            b'1' | b'2' | b'3' | b'4' | b'5' | b'6' | b'7' | b'8' | b'9' => {
                w.write_all(&[first])?;
                loop {
                    let Some(c) = input.try_peek() else {
                        return Ok(());
                    };
                    let c = c?;
                    if c.is_ascii_digit() {
                        w.write_all(&[c])?;
                        input.skip();
                    } else {
                        break;
                    }
                }
            }
            _ => return Err(SyntaxError::InvalidValue.into()),
        }
        Ok(())
    }

    fn walk_digits<I: ByteStream, W: Write>(&mut self, input: &mut I, w: &mut W) -> ParserResult {
        let mut has_digit = false;
        loop {
            let Some(c) = input.try_peek() else {
                break;
            };
            let c = c?;
            if c.is_ascii_digit() {
                w.write_all(&[c])?;
                input.skip();
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

    fn walk_fraction<I: ByteStream, W: Write>(&mut self, input: &mut I, w: &mut W) -> ParserResult {
        let Some(first) = input.try_peek() else {
            return Ok(());
        };
        let first = first?;
        if first != b'.' {
            return Ok(());
        }
        w.write_all(b".")?;
        input.skip();
        self.walk_digits(input, w)
    }

    fn walk_exponent<I: ByteStream, W: Write>(&mut self, input: &mut I, w: &mut W) -> ParserResult {
        let Some(first) = input.try_peek() else {
            return Ok(());
        };
        let first = first?;
        if first != b'e' && first != b'E' {
            return Ok(());
        }
        w.write_all(&[first])?;
        input.skip();
        self.walk_sign(input, w)?;
        self.walk_digits(input, w)
    }

    fn walk_sign<I: ByteStream, W: Write>(&mut self, input: &mut I, w: &mut W) -> ParserResult {
        let c = input.peek()??;
        if c == b'+' || c == b'-' {
            w.write_all(&[c])?;
            input.skip();
        }
        Ok(())
    }

    fn walk_ws<I: ByteStream, W: Write>(&mut self, input: &mut I, w: &mut W) -> ParserResult {
        loop {
            let Some(c) = input.try_peek() else {
                return Ok(());
            };
            let c = c?;
            match c {
                0x09 | 0x0A | 0x0D | 0x20 => {
                    w.write_all(&[c])?;
                    input.skip();
                }
                _ => return Ok(()),
            }
        }
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
