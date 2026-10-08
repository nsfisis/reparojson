use std::ffi::OsStr;
use std::fs::File;
use std::io::{BufReader, Read, Write, stdin};
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

pub fn repair_file(input_file_path: Option<&OsStr>, mut w: impl Write) -> RepairResult {
    match input_file_path {
        None => {
            let reader = stdin().lock();
            let reader = BufReader::new(reader);
            repair(reader, &mut w)
        }
        Some(file_path) => {
            if file_path == OsStr::new("-") {
                let reader = stdin().lock();
                let reader = BufReader::new(reader);
                repair(reader, &mut w)
            } else {
                let reader = File::open(file_path)?;
                let reader = BufReader::new(reader);
                repair(reader, &mut w)
            }
        }
    }
}

pub fn repair_file_in_place(file_path: &OsStr) -> RepairResult {
    // Buffer the whole output so that the file is left untouched on failure.
    let mut output = Vec::new();
    let result = repair_file(Some(file_path), &mut output)?;
    if matches!(result, RepairOk::Repaired) {
        std::fs::write(file_path, output)?;
    }
    Ok(result)
}

struct Parser<'input, 'output, I: ByteStream, W: Write> {
    input: &'input mut I,
    output: &'output mut W,
    repaired: bool,
    // True if the input ends where unclosed objects and arrays can be closed.
    truncated: bool,
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
            truncated: false,
        }
    }

    fn repaired(&self) -> bool {
        self.repaired
    }

    fn walk_json(&mut self) -> ParserResult {
        self.walk_bom()?;
        match self.walk_element() {
            Err(RepairErr::Invalid(SyntaxError::UnexpectedEof)) if self.truncated => {
                self.repaired = true;
                return Ok(());
            }
            res => res?,
        }
        if self.input.eof() {
            Ok(())
        } else {
            Err(SyntaxError::TrailingData.into())
        }
    }

    fn walk_bom(&mut self) -> ParserResult {
        let Some(first) = self.input.try_peek() else {
            return Ok(());
        };
        if first? != 0xEF {
            return Ok(());
        }
        // Remove UTF-8 BOM.
        self.input.skip();
        if self.input.next()?? != 0xBB || self.input.next()?? != 0xBF {
            return Err(SyntaxError::InvalidValue.into());
        }
        self.repaired = true;
        Ok(())
    }

    fn walk_value(&mut self) -> ParserResult {
        // Closers of the objects and arrays that are not closed yet. Nested values are walked
        // with this explicit stack instead of recursion, so that deeply nested input does not
        // overflow the call stack.
        let mut closers = Vec::new();
        let res = self.walk_value_body(&mut closers);
        if self.truncated && res.is_err() {
            // Close unclosed objects and arrays. Nothing else is inserted.
            closers.reverse();
            self.output.write_all(&closers)?;
        }
        res
    }

    #[inline(always)]
    fn walk_value_body(&mut self, closers: &mut Vec<u8>) -> ParserResult {
        let mut ws = Vec::with_capacity(1024);
        loop {
            let c = self.input.peek()??;
            match c {
                b'{' | b'[' => {
                    let closer = if c == b'{' { b'}' } else { b']' };
                    self.output.write_all(&[c])?;
                    self.input.skip(); // => { or [
                    closers.push(closer);

                    self.walk_ws()?;

                    // leading_comma_opt
                    let mut first = self.peek_token()?;
                    if first == b',' {
                        self.repaired = true;
                        self.input.skip();
                        self.walk_ws()?;
                        first = self.peek_token()?;
                    }

                    // members_opt or elements_opt
                    let has_items = if closer == b'}' {
                        first == b'"'
                    } else {
                        first != b']'
                    };
                    if has_items {
                        if closer == b'}' {
                            self.walk_member_key()?;
                        }
                        // Walk the first value in the object or array.
                        continue;
                    }

                    self.walk_char_of(closer)?;
                    closers.pop();
                }
                _ => self.walk_scalar()?,
            }

            // A value ends here. Close the objects and arrays that end with it.
            loop {
                let Some(&closer) = closers.last() else {
                    return Ok(());
                };
                if self.walk_separator(closer, &mut ws)? {
                    if closer == b'}' {
                        self.walk_member_key()?;
                    }
                    // Walk the next value in the object or array.
                    break;
                }
                self.walk_char_of(closer)?;
                closers.pop();
            }
        }
    }

    fn walk_scalar(&mut self) -> ParserResult {
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
            b'"' => self.walk_string(),
            b'-' | b'+' | b'.' => self.walk_number(),
            c if c.is_ascii_digit() => self.walk_number(),
            _ => Err(SyntaxError::InvalidValue.into()),
        }
    }

    /// Walks a separator between members or elements. Returns true if another member or element
    /// follows, or false if the object or array ends.
    fn walk_separator(&mut self, closer: u8, ws: &mut Vec<u8>) -> Result<bool, RepairErr> {
        ws.clear();
        self.walk_ws_with_buf(ws)?;

        let next = self.peek_after_ws(ws)?;
        if next == closer {
            self.output.write_all(ws)?;
            return Ok(false);
        }
        if next != b',' {
            // Insert a missing comma.
            self.repaired = true;
            self.output.write_all(b",")?;
            self.output.write_all(ws)?;
            return Ok(true);
        }

        self.output.write_all(ws)?;
        ws.clear();

        self.input.skip();

        self.walk_ws_with_buf(ws)?;

        let mut c = self.peek_after_ws(ws)?;
        while c == b',' {
            // Remove a duplicate comma.
            self.repaired = true;
            self.input.skip();
            self.walk_ws_with_buf(ws)?;
            c = self.peek_after_ws(ws)?;
        }
        if c == closer {
            // Remove a trailing comma.
            self.repaired = true;
            self.output.write_all(ws)?;
            return Ok(false);
        }
        self.output.write_all(b",")?;
        self.output.write_all(ws)?;
        Ok(true)
    }

    /// Walks a key of an object member and the following colon.
    fn walk_member_key(&mut self) -> ParserResult {
        if self.input.peek()?? != b'"' {
            return Err(SyntaxError::InvalidValue.into());
        }
        self.walk_string()?;

        let mut ws = Vec::new();
        self.walk_ws_with_buf(&mut ws)?;

        let maybe_colon = self.input.peek()??;
        if maybe_colon == b':' {
            self.output.write_all(&ws)?;
            self.input.skip();
            self.output.write_all(b":")?;
        } else {
            self.repaired = true;
            self.output.write_all(b":")?;
            self.output.write_all(&ws)?;
        }

        self.walk_ws()
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
                c if c < 0x20 => {
                    // A raw byte less than 0x20 cannot be embedded in string. Escape it.
                    self.repaired = true;
                    match c {
                        0x08 => self.output.write_all(b"\\b")?,
                        0x09 => self.output.write_all(b"\\t")?,
                        0x0A => self.output.write_all(b"\\n")?,
                        0x0C => self.output.write_all(b"\\f")?,
                        0x0D => self.output.write_all(b"\\r")?,
                        _ => write!(self.output, "\\u{:04x}", c)?,
                    }
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
        if self.input.peek()?? == b'+' {
            // Remove a leading plus sign.
            self.repaired = true;
            self.input.skip();
        }
        self.walk_integer()?;
        self.walk_fraction()?;
        self.walk_exponent()
    }

    fn walk_integer(&mut self) -> ParserResult {
        let first = self.input.peek()??;
        match first {
            b'-' => {
                self.input.skip();
                self.output.write_all(b"-")?;
                return self.walk_integer();
            }
            b'.' => {
                // Insert a missing integer part. The fraction part follows.
                self.repaired = true;
                self.output.write_all(b"0")?;
                return Ok(());
            }
            b'0' => {
                self.input.skip();
                while let Some(c) = self.input.try_peek() {
                    match c? {
                        // Remove leading zeros.
                        b'0' => {
                            self.repaired = true;
                            self.input.skip();
                        }
                        c if c.is_ascii_digit() => {
                            self.repaired = true;
                            return self.walk_integer();
                        }
                        _ => break,
                    }
                }
                self.output.write_all(b"0")?;
                return Ok(());
            }
            b'1' | b'2' | b'3' | b'4' | b'5' | b'6' | b'7' | b'8' | b'9' => {
                self.input.skip();
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

    /// Peeks the next byte where an object or array can be closed.
    fn peek_token(&mut self) -> Result<u8, RepairErr> {
        self.peek_after_ws(&[])
    }

    /// Same as `peek_token()`, but flushes the pending whitespaces on the end of file.
    fn peek_after_ws(&mut self, ws: &[u8]) -> Result<u8, RepairErr> {
        match self.input.try_peek() {
            Some(c) => Ok(c?),
            None => {
                self.output.write_all(ws)?;
                self.truncated = true;
                Err(SyntaxError::UnexpectedEof.into())
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
        assert!(repair(r#"."#).0.is_err());
        assert!(repair(r#"+"#).0.is_err());
        assert!(repair(r#"++1"#).0.is_err());
        assert!(repair(r#"-+1"#).0.is_err());
        assert!(repair(r#""#).0.is_err());
        assert!(repair(r#""a"#).0.is_err());
        assert!(repair(r#"["a"#).0.is_err());
        assert!(repair(r#"{"a"#).0.is_err());
        assert!(repair(r#"[tru"#).0.is_err());
        assert!(repair(r#"[1e"#).0.is_err());
        assert!(repair(r#"[-"#).0.is_err());
        assert!(repair(r#"   "#).0.is_err());
        assert!(repair(r#"[[1,2, {"a":"#).0.is_err());
        assert!(repair(r#"{"a": 1, "b" "#).0.is_err());
        assert!(repair(r#"{"a": {"b": [1], "c": "#).0.is_err());
        assert!(repair(r#"{"a":1, b":2}"#).0.is_err());
        assert!(repair(r#"{"a":1 b":2}"#).0.is_err());
        assert!(repair(r#"{"a":1, 2}"#).0.is_err());
        assert!(repair(r#"{"a":1]"#).0.is_err());
        assert!(repair(r#"[1}"#).0.is_err());
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
    fn test_repair_bom() {
        {
            let s = "\u{FEFF}[1, 2]";
            let (res, out) = repair(s);
            assert!(matches!(res, Ok(super::RepairOk::Repaired)));
            assert_eq!("[1, 2]", out);
        }
        {
            let s = "\u{FEFF} {\"a\": \"\u{FEFF}\"}\n";
            let (res, out) = repair(s);
            assert!(matches!(res, Ok(super::RepairOk::Repaired)));
            assert_eq!(" {\"a\": \"\u{FEFF}\"}\n", out);
        }
        assert!(repair("\u{FEFF}").0.is_err());
        assert!(repair("\u{FEFF}\u{FEFF}1").0.is_err());
        assert!(repair("1\u{FEFF}").0.is_err());
        {
            let mut output = Vec::new();
            let result = super::repair(&b"\xEF\xBB1"[..], &mut output);
            assert!(result.is_err());
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
        {
            let s = r#"{"a" 1}"#;
            let (res, out) = repair(s);
            assert!(matches!(res, Ok(super::RepairOk::Repaired)));
            assert_eq!(r#"{"a": 1}"#, out);
        }
        {
            let s = r#"{"a"1  "b"  2}"#;
            let (res, out) = repair(s);
            assert!(matches!(res, Ok(super::RepairOk::Repaired)));
            assert_eq!(r#"{"a":1,  "b":  2}"#, out);
        }
        {
            let s = "[\"a\tb\"]";
            let (res, out) = repair(s);
            assert!(matches!(res, Ok(super::RepairOk::Repaired)));
            assert_eq!(r#"["a\tb"]"#, out);
        }
        {
            let s = "[\"a\nb\r\nc\u{08}\u{0C}\"]";
            let (res, out) = repair(s);
            assert!(matches!(res, Ok(super::RepairOk::Repaired)));
            assert_eq!(r#"["a\nb\r\nc\b\f"]"#, out);
        }
        {
            let s = "{\"\u{00}\": \"\u{01}\u{1F}\u{7F}\"}";
            let (res, out) = repair(s);
            assert!(matches!(res, Ok(super::RepairOk::Repaired)));
            assert_eq!("{\"\\u0000\": \"\\u0001\\u001f\u{7F}\"}", out);
        }
        {
            let s = r#"[,1]"#;
            let (res, out) = repair(s);
            assert!(matches!(res, Ok(super::RepairOk::Repaired)));
            assert_eq!("[1]", out);
        }
        {
            let s = r#"[ ,  1 2,]"#;
            let (res, out) = repair(s);
            assert!(matches!(res, Ok(super::RepairOk::Repaired)));
            assert_eq!("[   1, 2]", out);
        }
        {
            let s = r#"[1,,2]"#;
            let (res, out) = repair(s);
            assert!(matches!(res, Ok(super::RepairOk::Repaired)));
            assert_eq!("[1,2]", out);
        }
        {
            let s = r#"[1 , , 2,, ,3,,]"#;
            let (res, out) = repair(s);
            assert!(matches!(res, Ok(super::RepairOk::Repaired)));
            assert_eq!("[1 ,  2, 3]", out);
        }
        {
            let s = r#".3"#;
            let (res, out) = repair(s);
            assert!(matches!(res, Ok(super::RepairOk::Repaired)));
            assert_eq!("0.3", out);
        }
        {
            let s = r#"[-.5e1, .25]"#;
            let (res, out) = repair(s);
            assert!(matches!(res, Ok(super::RepairOk::Repaired)));
            assert_eq!("[-0.5e1, 0.25]", out);
        }
        {
            let s = r#"+1"#;
            let (res, out) = repair(s);
            assert!(matches!(res, Ok(super::RepairOk::Repaired)));
            assert_eq!("1", out);
        }
        {
            let s = r#"[+1.5e+2, +0]"#;
            let (res, out) = repair(s);
            assert!(matches!(res, Ok(super::RepairOk::Repaired)));
            assert_eq!("[1.5e+2, 0]", out);
        }
        {
            let s = r#"0123"#;
            let (res, out) = repair(s);
            assert!(matches!(res, Ok(super::RepairOk::Repaired)));
            assert_eq!("123", out);
        }
        {
            let s = r#"[000, -007, 00.50, 0e1]"#;
            let (res, out) = repair(s);
            assert!(matches!(res, Ok(super::RepairOk::Repaired)));
            assert_eq!("[0, -7, 0.50, 0e1]", out);
        }
        {
            let s = r#"[[1,2, {"a":[3,"#;
            let (res, out) = repair(s);
            assert!(matches!(res, Ok(super::RepairOk::Repaired)));
            assert_eq!(r#"[[1,2, {"a":[3]}]]"#, out);
        }
        {
            let s = "[\n  1,\n  {\n    \"a\": [2 ,\n";
            let (res, out) = repair(s);
            assert!(matches!(res, Ok(super::RepairOk::Repaired)));
            assert_eq!("[\n  1,\n  {\n    \"a\": [2 \n]}]", out);
        }
        {
            let s = r#"{"a": 1, "#;
            let (res, out) = repair(s);
            assert!(matches!(res, Ok(super::RepairOk::Repaired)));
            assert_eq!(r#"{"a": 1 }"#, out);
        }
        {
            let s = r#"[{"#;
            let (res, out) = repair(s);
            assert!(matches!(res, Ok(super::RepairOk::Repaired)));
            assert_eq!("[{}]", out);
        }
        {
            let s = r#"{,"a":1}"#;
            let (res, out) = repair(s);
            assert!(matches!(res, Ok(super::RepairOk::Repaired)));
            assert_eq!(r#"{"a":1}"#, out);
        }
        {
            let s = r#"{ ,  "a":1 "b":2,}"#;
            let (res, out) = repair(s);
            assert!(matches!(res, Ok(super::RepairOk::Repaired)));
            assert_eq!(r#"{   "a":1, "b":2}"#, out);
        }
        {
            let s = r#"{"a":1,,"b":2}"#;
            let (res, out) = repair(s);
            assert!(matches!(res, Ok(super::RepairOk::Repaired)));
            assert_eq!(r#"{"a":1,"b":2}"#, out);
        }
        {
            let s = r#"{"a":1 , , "b":2,, ,"c":3,,}"#;
            let (res, out) = repair(s);
            assert!(matches!(res, Ok(super::RepairOk::Repaired)));
            assert_eq!(r#"{"a":1 ,  "b":2, "c":3}"#, out);
        }
    }
}
