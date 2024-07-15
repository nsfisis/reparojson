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

pub enum SyntaxError {
    UnexpectedEof,
    InvalidValue,
}

impl SyntaxError {
    fn to_result(self) -> ParserResult {
        Err(RepairErr::Invalid(self))
    }
}

impl std::fmt::Display for SyntaxError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::UnexpectedEof => write!(f, "unexpected end of file"),
            Self::InvalidValue => write!(f, "invalid value"),
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

impl Parser {
    fn new() -> Self {
        Self { repaired: false }
    }

    fn repaired(&self) -> bool {
        self.repaired
    }

    fn walk_json<I: Iterator<Item = std::io::Result<u8>>, W: Write>(
        &mut self,
        input: &mut Peekable<I>,
        w: &mut W,
    ) -> ParserResult {
        self.walk_element(input, w)
    }

    fn walk_value<I: Iterator<Item = std::io::Result<u8>>, W: Write>(
        &mut self,
        input: &mut Peekable<I>,
        w: &mut W,
    ) -> ParserResult {
        let Some(c) = input.peek() else {
            return SyntaxError::UnexpectedEof.to_result();
        };
        let Ok(c) = c else {
            return Err(input.next().unwrap().unwrap_err().into());
        };

        match c {
            b'n' => {
                input.next(); // => n
                match input.next() {
                    Some(Ok(b'u')) => (),
                    Some(Ok(_)) => return SyntaxError::InvalidValue.to_result(),
                    Some(Err(err)) => return Err(err.into()),
                    None => return SyntaxError::UnexpectedEof.to_result(),
                }
                match input.next() {
                    Some(Ok(b'l')) => (),
                    Some(Ok(_)) => return SyntaxError::InvalidValue.to_result(),
                    Some(Err(err)) => return Err(err.into()),
                    None => return SyntaxError::UnexpectedEof.to_result(),
                }
                match input.next() {
                    Some(Ok(b'l')) => (),
                    Some(Ok(_)) => return SyntaxError::InvalidValue.to_result(),
                    Some(Err(err)) => return Err(err.into()),
                    None => return SyntaxError::UnexpectedEof.to_result(),
                }
                w.write_all(b"null")?;
                Ok(())
            }
            b't' => {
                input.next(); // => t
                match input.next() {
                    Some(Ok(b'r')) => (),
                    Some(Ok(_)) => return SyntaxError::InvalidValue.to_result(),
                    Some(Err(err)) => return Err(err.into()),
                    None => return SyntaxError::UnexpectedEof.to_result(),
                }
                match input.next() {
                    Some(Ok(b'u')) => (),
                    Some(Ok(_)) => return SyntaxError::InvalidValue.to_result(),
                    Some(Err(err)) => return Err(err.into()),
                    None => return SyntaxError::UnexpectedEof.to_result(),
                }
                match input.next() {
                    Some(Ok(b'e')) => (),
                    Some(Ok(_)) => return SyntaxError::InvalidValue.to_result(),
                    Some(Err(err)) => return Err(err.into()),
                    None => return SyntaxError::UnexpectedEof.to_result(),
                }
                w.write_all(b"true")?;
                Ok(())
            }
            b'f' => {
                input.next(); // => f
                match input.next() {
                    Some(Ok(b'a')) => (),
                    Some(Ok(_)) => return SyntaxError::InvalidValue.to_result(),
                    Some(Err(err)) => return Err(err.into()),
                    None => return SyntaxError::UnexpectedEof.to_result(),
                }
                match input.next() {
                    Some(Ok(b'l')) => (),
                    Some(Ok(_)) => return SyntaxError::InvalidValue.to_result(),
                    Some(Err(err)) => return Err(err.into()),
                    None => return SyntaxError::UnexpectedEof.to_result(),
                }
                match input.next() {
                    Some(Ok(b's')) => (),
                    Some(Ok(_)) => return SyntaxError::InvalidValue.to_result(),
                    Some(Err(err)) => return Err(err.into()),
                    None => return SyntaxError::UnexpectedEof.to_result(),
                }
                match input.next() {
                    Some(Ok(b'e')) => (),
                    Some(Ok(_)) => return SyntaxError::InvalidValue.to_result(),
                    Some(Err(err)) => return Err(err.into()),
                    None => return SyntaxError::UnexpectedEof.to_result(),
                }
                w.write_all(b"false")?;
                Ok(())
            }
            b'{' => self.walk_object(input, w),
            b'[' => self.walk_array(input, w),
            b'"' => self.walk_string(input, w),
            b'-' | b'0' | b'1' | b'2' | b'3' | b'4' | b'5' | b'6' | b'7' | b'8' | b'9' => {
                self.walk_number(input, w)
            }
            _ => SyntaxError::InvalidValue.to_result(),
        }
    }

    fn walk_object<I: Iterator<Item = std::io::Result<u8>>, W: Write>(
        &mut self,
        input: &mut Peekable<I>,
        w: &mut W,
    ) -> ParserResult {
        w.write_all(b"{")?;
        input.next(); // => {

        self.walk_ws(input, w)?;

        // members_opt
        let Some(first) = input.peek() else {
            return SyntaxError::UnexpectedEof.to_result();
        };
        let Ok(first) = first else {
            return Err(input.next().unwrap().unwrap_err().into());
        };
        if *first == b'"' {
            self.walk_members(input, w)?;
        }

        // trailing_comma_opt
        let Some(maybe_comma) = input.peek() else {
            return SyntaxError::UnexpectedEof.to_result();
        };
        let Ok(maybe_comma) = maybe_comma else {
            return Err(input.next().unwrap().unwrap_err().into());
        };
        if *maybe_comma == b',' {
            self.repaired = true;
            input.next();
            self.walk_ws(input, w)?;
        }

        let Some(last) = input.next() else {
            return SyntaxError::UnexpectedEof.to_result();
        };
        let last = last?;
        if last != b'}' {
            return SyntaxError::InvalidValue.to_result();
        }
        w.write_all(b"}")?;
        Ok(())
    }

    fn walk_members<I: Iterator<Item = std::io::Result<u8>>, W: Write>(
        &mut self,
        input: &mut Peekable<I>,
        w: &mut W,
    ) -> ParserResult {
        loop {
            self.walk_member(input, w)?;

            let mut ws = Vec::with_capacity(1024);
            self.walk_ws(input, &mut ws)?;

            let Some(next) = input.peek() else {
                return SyntaxError::UnexpectedEof.to_result();
            };
            let Ok(next) = next else {
                return Err(input.next().unwrap().unwrap_err().into());
            };

            match *next {
                b'}' => {
                    w.write_all(&mut ws)?;
                    return Ok(());
                }
                b',' => {
                    w.write_all(&mut ws)?;

                    input.next();

                    self.walk_ws(input, &mut ws)?;

                    let Some(c) = input.peek() else {
                        return SyntaxError::UnexpectedEof.to_result();
                    };
                    let Ok(c) = c else {
                        return Err(input.next().unwrap().unwrap_err().into());
                    };
                    match *c {
                        b'}' => {
                            self.repaired = true;
                            w.write_all(&mut ws)?;
                            return Ok(());
                        }
                        _ => {
                            w.write_all(b",")?;
                            w.write_all(&mut ws)?;
                        }
                    }
                }
                _ => {
                    self.repaired = true;
                    w.write_all(b",")?;
                    w.write_all(&mut ws)?;
                }
            }
        }
    }

    fn walk_member<I: Iterator<Item = std::io::Result<u8>>, W: Write>(
        &mut self,
        input: &mut Peekable<I>,
        w: &mut W,
    ) -> ParserResult {
        self.walk_string(input, w)?;
        self.walk_ws(input, w)?;
        let Some(colon) = input.next() else {
            return SyntaxError::UnexpectedEof.to_result();
        };
        let colon = colon?;
        if colon != b':' {
            return SyntaxError::InvalidValue.to_result();
        }
        w.write_all(b":")?;
        self.walk_element(input, w)
    }

    fn walk_array<I: Iterator<Item = std::io::Result<u8>>, W: Write>(
        &mut self,
        input: &mut Peekable<I>,
        w: &mut W,
    ) -> ParserResult {
        w.write_all(b"[")?;
        input.next(); // => [

        self.walk_ws(input, w)?;

        // elements_opt
        let Some(first) = input.peek() else {
            return SyntaxError::UnexpectedEof.to_result();
        };
        let Ok(first) = first else {
            return Err(input.next().unwrap().unwrap_err().into());
        };
        if *first != b',' && *first != b']' {
            self.walk_elements(input, w)?;
        }

        // trailing_comma_opt
        let Some(maybe_comma) = input.peek() else {
            return SyntaxError::UnexpectedEof.to_result();
        };
        let Ok(maybe_comma) = maybe_comma else {
            return Err(input.next().unwrap().unwrap_err().into());
        };
        if *maybe_comma == b',' {
            self.repaired = true;
            input.next();
            self.walk_ws(input, w)?;
        }

        let Some(last) = input.next() else {
            return SyntaxError::UnexpectedEof.to_result();
        };
        let last = last?;
        if last != b']' {
            return SyntaxError::InvalidValue.to_result();
        }
        w.write_all(b"]")?;
        Ok(())
    }

    fn walk_elements<I: Iterator<Item = std::io::Result<u8>>, W: Write>(
        &mut self,
        input: &mut Peekable<I>,
        w: &mut W,
    ) -> ParserResult {
        loop {
            self.walk_value(input, w)?;

            let mut ws = Vec::with_capacity(1024);
            self.walk_ws(input, &mut ws)?;

            let Some(next) = input.peek() else {
                return SyntaxError::UnexpectedEof.to_result();
            };
            let Ok(next) = next else {
                return Err(input.next().unwrap().unwrap_err().into());
            };

            match *next {
                b']' => {
                    w.write_all(&mut ws)?;
                    return Ok(());
                }
                b',' => {
                    w.write_all(&mut ws)?;

                    input.next();

                    self.walk_ws(input, &mut ws)?;

                    let Some(c) = input.peek() else {
                        return SyntaxError::UnexpectedEof.to_result();
                    };
                    let Ok(c) = c else {
                        return Err(input.next().unwrap().unwrap_err().into());
                    };
                    match *c {
                        b']' => {
                            self.repaired = true;
                            w.write_all(&mut ws)?;
                            return Ok(());
                        }
                        _ => {
                            w.write_all(b",")?;
                            w.write_all(&mut ws)?;
                        }
                    }
                }
                _ => {
                    self.repaired = true;
                    w.write_all(b",")?;
                    w.write_all(&mut ws)?;
                }
            }
        }
    }

    fn walk_element<I: Iterator<Item = std::io::Result<u8>>, W: Write>(
        &mut self,
        input: &mut Peekable<I>,
        w: &mut W,
    ) -> ParserResult {
        self.walk_ws(input, w)?;
        self.walk_value(input, w)?;
        self.walk_ws(input, w)
    }

    fn walk_string<I: Iterator<Item = std::io::Result<u8>>, W: Write>(
        &mut self,
        input: &mut Peekable<I>,
        w: &mut W,
    ) -> ParserResult {
        w.write_all(b"\"")?;
        input.next(); // => "
        loop {
            match input.next() {
                Some(Ok(b'"')) => break,
                Some(Ok(b'\\')) => {
                    self.walk_escape(input, w)?;
                }
                Some(Ok(c)) => {
                    w.write_all(&[c])?;
                }
                Some(Err(_)) => return Err(input.next().unwrap().unwrap_err().into()),
                None => return SyntaxError::UnexpectedEof.to_result(),
            }
        }
        w.write_all(b"\"")?;
        Ok(())
    }

    fn walk_escape<I: Iterator<Item = std::io::Result<u8>>, W: Write>(
        &mut self,
        input: &mut Peekable<I>,
        w: &mut W,
    ) -> ParserResult {
        let Some(c) = input.next() else {
            return SyntaxError::UnexpectedEof.to_result();
        };
        let c = c?;
        match c {
            b'"' | b'\\' | b'/' | b'b' | b'f' | b'n' | b'r' | b't' => {
                w.write_all(&[b'\\', c])?;
            }
            b'u' => {
                let Some(u1) = input.next() else {
                    return SyntaxError::UnexpectedEof.to_result();
                };
                let u1 = u1?;
                if !u1.is_ascii_hexdigit() {
                    return SyntaxError::InvalidValue.to_result();
                }
                let Some(u2) = input.next() else {
                    return SyntaxError::UnexpectedEof.to_result();
                };
                let u2 = u2?;
                if !u2.is_ascii_hexdigit() {
                    return SyntaxError::InvalidValue.to_result();
                }
                let Some(u3) = input.next() else {
                    return SyntaxError::UnexpectedEof.to_result();
                };
                let u3 = u3?;
                if !u3.is_ascii_hexdigit() {
                    return SyntaxError::InvalidValue.to_result();
                }
                let Some(u4) = input.next() else {
                    return SyntaxError::UnexpectedEof.to_result();
                };
                let u4 = u4?;
                if !u4.is_ascii_hexdigit() {
                    return SyntaxError::InvalidValue.to_result();
                }
                w.write_all(&[b'\\', u1, u2, u3, u4])?;
            }
            _ => return SyntaxError::InvalidValue.to_result(),
        }
        Ok(())
    }

    fn walk_number<I: Iterator<Item = std::io::Result<u8>>, W: Write>(
        &mut self,
        input: &mut Peekable<I>,
        w: &mut W,
    ) -> ParserResult {
        self.walk_integer(input, w)?;
        self.walk_fraction(input, w)?;
        self.walk_exponent(input, w)
    }

    fn walk_integer<I: Iterator<Item = std::io::Result<u8>>, W: Write>(
        &mut self,
        input: &mut Peekable<I>,
        w: &mut W,
    ) -> ParserResult {
        let Some(first) = input.next() else {
            return SyntaxError::UnexpectedEof.to_result();
        };
        let first = first?;
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
                    match input.peek() {
                        Some(Ok(c @ b'0')) | Some(Ok(c @ b'1')) | Some(Ok(c @ b'2'))
                        | Some(Ok(c @ b'3')) | Some(Ok(c @ b'4')) | Some(Ok(c @ b'5'))
                        | Some(Ok(c @ b'6')) | Some(Ok(c @ b'7')) | Some(Ok(c @ b'8'))
                        | Some(Ok(c @ b'9')) => {
                            w.write_all(&[*c])?;
                            input.next();
                        }
                        Some(Ok(_)) => break,
                        Some(Err(_)) => return Err(input.next().unwrap().unwrap_err().into()),
                        None => return Ok(()),
                    }
                }
            }
            _ => return SyntaxError::InvalidValue.to_result(),
        }
        Ok(())
    }

    fn walk_digits<I: Iterator<Item = std::io::Result<u8>>, W: Write>(
        &mut self,
        input: &mut Peekable<I>,
        w: &mut W,
    ) -> ParserResult {
        let mut has_digit = false;
        loop {
            match input.peek() {
                Some(Ok(c @ b'0')) | Some(Ok(c @ b'1')) | Some(Ok(c @ b'2'))
                | Some(Ok(c @ b'3')) | Some(Ok(c @ b'4')) | Some(Ok(c @ b'5'))
                | Some(Ok(c @ b'6')) | Some(Ok(c @ b'7')) | Some(Ok(c @ b'8'))
                | Some(Ok(c @ b'9')) => {
                    w.write_all(&[*c])?;
                    input.next();
                    has_digit = true;
                }
                Some(Ok(_)) => break,
                Some(Err(_)) => return Err(input.next().unwrap().unwrap_err().into()),
                None => break,
            }
        }
        if has_digit {
            Ok(())
        } else {
            match input.peek() {
                Some(_) => SyntaxError::InvalidValue.to_result(),
                None => SyntaxError::UnexpectedEof.to_result(),
            }
        }
    }

    fn walk_fraction<I: Iterator<Item = std::io::Result<u8>>, W: Write>(
        &mut self,
        input: &mut Peekable<I>,
        w: &mut W,
    ) -> ParserResult {
        let Some(first) = input.peek() else {
            return Ok(());
        };
        let Ok(first) = first else {
            return Err(input.next().unwrap().unwrap_err().into());
        };
        if *first != b'.' {
            return Ok(());
        }
        w.write_all(b".")?;
        input.next();
        self.walk_digits(input, w)
    }

    fn walk_exponent<I: Iterator<Item = std::io::Result<u8>>, W: Write>(
        &mut self,
        input: &mut Peekable<I>,
        w: &mut W,
    ) -> ParserResult {
        let Some(first) = input.peek() else {
            return Ok(());
        };
        let Ok(first) = first else {
            return Err(input.next().unwrap().unwrap_err().into());
        };
        if *first != b'e' && *first != b'E' {
            return Ok(());
        }
        w.write_all(&[*first])?;
        input.next();
        self.walk_sign(input, w)?;
        self.walk_digits(input, w)
    }

    fn walk_sign<I: Iterator<Item = std::io::Result<u8>>, W: Write>(
        &mut self,
        input: &mut Peekable<I>,
        w: &mut W,
    ) -> ParserResult {
        let Some(c) = input.peek() else {
            return SyntaxError::UnexpectedEof.to_result();
        };
        let Ok(c) = c else {
            return Err(input.next().unwrap().unwrap_err().into());
        };
        if *c == b'+' || *c == b'-' {
            w.write_all(&[*c])?;
            input.next();
        }
        Ok(())
    }

    fn walk_ws<I: Iterator<Item = std::io::Result<u8>>, W: Write>(
        &mut self,
        input: &mut Peekable<I>,
        w: &mut W,
    ) -> ParserResult {
        loop {
            match input.peek() {
                Some(Ok(c @ 0x09)) | Some(Ok(c @ 0x0A)) | Some(Ok(c @ 0x0D))
                | Some(Ok(c @ 0x20)) => {
                    w.write_all(&[*c])?;
                    input.next();
                }
                Some(Ok(_)) => return Ok(()),
                Some(Err(_)) => return Err(input.next().unwrap().unwrap_err().into()),
                None => return Ok(()),
            }
        }
    }
}
