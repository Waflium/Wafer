#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Strictness {
    Lenient,
    LegacyStrict,
    Strict,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JsonToken {
    BeginArray,
    EndArray,
    BeginObject,
    EndObject,
    Name,
    String,
    Number,
    Boolean,
    Null,
    EndDocument,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum JsonReaderError {
    EndOfInput(String),
    Malformed(String),
    IllegalState(String),
}

type Result<T> = std::result::Result<T, JsonReaderError>;

const PEEKED_NONE: u8 = 0;
const PEEKED_BEGIN_OBJECT: u8 = 1;
const PEEKED_END_OBJECT: u8 = 2;
const PEEKED_BEGIN_ARRAY: u8 = 3;
const PEEKED_END_ARRAY: u8 = 4;
const PEEKED_TRUE: u8 = 5;
const PEEKED_FALSE: u8 = 6;
const PEEKED_NULL: u8 = 7;
const PEEKED_SINGLE_QUOTED: u8 = 8;
const PEEKED_DOUBLE_QUOTED: u8 = 9;
const PEEKED_UNQUOTED: u8 = 10;
const PEEKED_BUFFERED: u8 = 11;
const PEEKED_SINGLE_QUOTED_NAME: u8 = 12;
const PEEKED_DOUBLE_QUOTED_NAME: u8 = 13;
const PEEKED_UNQUOTED_NAME: u8 = 14;
const PEEKED_LONG: u8 = 15;
const PEEKED_NUMBER: u8 = 16;
const PEEKED_EOF: u8 = 17;

const NUMBER_CHAR_NONE: u8 = 0;
const NUMBER_CHAR_SIGN: u8 = 1;
const NUMBER_CHAR_DIGIT: u8 = 2;
const NUMBER_CHAR_DECIMAL: u8 = 3;
const NUMBER_CHAR_FRACTION_DIGIT: u8 = 4;
const NUMBER_CHAR_EXP_E: u8 = 5;
const NUMBER_CHAR_EXP_SIGN: u8 = 6;
const NUMBER_CHAR_EXP_DIGIT: u8 = 7;

const MIN_INCOMPLETE_INTEGER: i64 = i64::MIN / 10;
const BUFFER_LENGTH: usize = 1024;

const EMPTY_ARRAY: u8 = 1;
const NONEMPTY_ARRAY: u8 = 2;
const EMPTY_OBJECT: u8 = 3;
const DANGLING_NAME: u8 = 4;
const NONEMPTY_OBJECT: u8 = 5;
const EMPTY_DOCUMENT: u8 = 6;
const NONEMPTY_DOCUMENT: u8 = 7;

pub struct JsonReader {
    pub strictness: Strictness,
    nesting_limit: usize,
    buffer: Vec<u16>,
    pos: usize,
    line_number: usize,
    line_start: usize,
    peeked: u8,
    peeked_long: i64,
    peeked_number_length: usize,
    peeked_string: Option<String>,
    stack: Vec<u8>,
    path_names: Vec<Option<String>>,
    path_indices: Vec<i32>,
}

impl JsonReader {
    pub fn new(input: &str) -> JsonReader {
        let buffer: Vec<u16> = input.encode_utf16().collect();
        let skip_bom = buffer.first() == Some(&0xfeff);
        JsonReader {
            strictness: Strictness::LegacyStrict,
            nesting_limit: 255,
            buffer,
            pos: skip_bom as usize,
            line_number: 0,
            line_start: skip_bom as usize,
            peeked: PEEKED_NONE,
            peeked_long: 0,
            peeked_number_length: 0,
            peeked_string: None,
            stack: vec![EMPTY_DOCUMENT],
            path_names: vec![None],
            path_indices: vec![0],
        }
    }

    fn limit(&self) -> usize {
        self.buffer.len()
    }

    fn peeked_or_peek(&mut self) -> Result<u8> {
        if self.peeked == PEEKED_NONE { self.do_peek() } else { Ok(self.peeked) }
    }

    fn top_index(&mut self) -> &mut i32 {
        let index = self.stack.len() - 1;
        &mut self.path_indices[index]
    }

    pub fn begin_array(&mut self) -> Result<()> {
        if self.peeked_or_peek()? == PEEKED_BEGIN_ARRAY {
            self.push(EMPTY_ARRAY)?;
            *self.top_index() = 0;
            self.peeked = PEEKED_NONE;
            Ok(())
        } else {
            Err(self.unexpected_token_error("BEGIN_ARRAY")?)
        }
    }

    pub fn end_array(&mut self) -> Result<()> {
        if self.peeked_or_peek()? == PEEKED_END_ARRAY {
            self.pop();
            *self.top_index() += 1;
            self.peeked = PEEKED_NONE;
            Ok(())
        } else {
            Err(self.unexpected_token_error("END_ARRAY")?)
        }
    }

    pub fn begin_object(&mut self) -> Result<()> {
        if self.peeked_or_peek()? == PEEKED_BEGIN_OBJECT {
            self.push(EMPTY_OBJECT)?;
            self.peeked = PEEKED_NONE;
            Ok(())
        } else {
            Err(self.unexpected_token_error("BEGIN_OBJECT")?)
        }
    }

    pub fn end_object(&mut self) -> Result<()> {
        if self.peeked_or_peek()? == PEEKED_END_OBJECT {
            self.pop();
            *self.top_index() += 1;
            self.peeked = PEEKED_NONE;
            Ok(())
        } else {
            Err(self.unexpected_token_error("END_OBJECT")?)
        }
    }

    pub fn has_next(&mut self) -> Result<bool> {
        let p = self.peeked_or_peek()?;
        Ok(p != PEEKED_END_OBJECT && p != PEEKED_END_ARRAY && p != PEEKED_EOF)
    }

    pub fn peek(&mut self) -> Result<JsonToken> {
        Ok(match self.peeked_or_peek()? {
            PEEKED_BEGIN_OBJECT => JsonToken::BeginObject,
            PEEKED_END_OBJECT => JsonToken::EndObject,
            PEEKED_BEGIN_ARRAY => JsonToken::BeginArray,
            PEEKED_END_ARRAY => JsonToken::EndArray,
            PEEKED_TRUE | PEEKED_FALSE => JsonToken::Boolean,
            PEEKED_NULL => JsonToken::Null,
            PEEKED_SINGLE_QUOTED | PEEKED_DOUBLE_QUOTED | PEEKED_UNQUOTED | PEEKED_BUFFERED => JsonToken::String,
            PEEKED_SINGLE_QUOTED_NAME | PEEKED_DOUBLE_QUOTED_NAME | PEEKED_UNQUOTED_NAME => JsonToken::Name,
            PEEKED_LONG | PEEKED_NUMBER => JsonToken::Number,
            PEEKED_EOF => JsonToken::EndDocument,
            _ => unreachable!(),
        })
    }

    fn do_peek(&mut self) -> Result<u8> {
        let top = self.stack.len() - 1;
        let peek_stack = self.stack[top];
        if peek_stack == EMPTY_ARRAY {
            self.stack[top] = NONEMPTY_ARRAY;
        } else if peek_stack == NONEMPTY_ARRAY {
            match self.next_non_whitespace(true)? {
                0x2c => {}
                0x3b => self.check_lenient()?,
                0x5d => {
                    self.peeked = PEEKED_END_ARRAY;
                    return Ok(self.peeked);
                }
                _ => return Err(self.syntax_error("Unterminated array")),
            }
        } else if peek_stack == EMPTY_OBJECT || peek_stack == NONEMPTY_OBJECT {
            self.stack[top] = DANGLING_NAME;
            if peek_stack == NONEMPTY_OBJECT {
                match self.next_non_whitespace(true)? {
                    0x2c => {}
                    0x3b => self.check_lenient()?,
                    0x7d => {
                        self.peeked = PEEKED_END_OBJECT;
                        return Ok(self.peeked);
                    }
                    _ => return Err(self.syntax_error("Unterminated object")),
                }
            }
            let c = self.next_non_whitespace(true)?;
            match c {
                0x22 => {
                    self.peeked = PEEKED_DOUBLE_QUOTED_NAME;
                    return Ok(self.peeked);
                }
                0x27 => {
                    self.check_lenient()?;
                    self.peeked = PEEKED_SINGLE_QUOTED_NAME;
                    return Ok(self.peeked);
                }
                0x7d => {
                    if peek_stack != NONEMPTY_OBJECT {
                        self.peeked = PEEKED_END_OBJECT;
                        return Ok(self.peeked);
                    }
                    return Err(self.syntax_error("Expected name"));
                }
                _ => {
                    self.check_lenient()?;
                    self.pos -= 1;
                    if self.is_literal(c as u16)? {
                        self.peeked = PEEKED_UNQUOTED_NAME;
                        return Ok(self.peeked);
                    }
                    return Err(self.syntax_error("Expected name"));
                }
            }
        } else if peek_stack == DANGLING_NAME {
            self.stack[top] = NONEMPTY_OBJECT;
            match self.next_non_whitespace(true)? {
                0x3a => {}
                0x3d => {
                    self.check_lenient()?;
                    if self.pos < self.limit() && self.buffer[self.pos] == b'>' as u16 {
                        self.pos += 1;
                    }
                }
                _ => return Err(self.syntax_error("Expected ':'")),
            }
        } else if peek_stack == EMPTY_DOCUMENT {
            if self.strictness == Strictness::Lenient {
                self.consume_non_execute_prefix()?;
            }
            self.stack[top] = NONEMPTY_DOCUMENT;
        } else if peek_stack == NONEMPTY_DOCUMENT {
            let c = self.next_non_whitespace(false)?;
            if c == -1 {
                self.peeked = PEEKED_EOF;
                return Ok(self.peeked);
            }
            self.check_lenient()?;
            self.pos -= 1;
        }

        let c = self.next_non_whitespace(true)?;
        match c {
            0x22 => {
                self.peeked = PEEKED_DOUBLE_QUOTED;
                Ok(self.peeked)
            }
            0x27 => {
                self.check_lenient()?;
                self.peeked = PEEKED_SINGLE_QUOTED;
                Ok(self.peeked)
            }
            0x5b => {
                self.peeked = PEEKED_BEGIN_ARRAY;
                Ok(self.peeked)
            }
            0x5d if peek_stack == EMPTY_ARRAY => {
                self.peeked = PEEKED_END_ARRAY;
                Ok(self.peeked)
            }
            0x5d | 0x2c | 0x3b => {
                if peek_stack != EMPTY_ARRAY && peek_stack != NONEMPTY_ARRAY {
                    return Err(self.syntax_error("Unexpected value"));
                }
                self.check_lenient()?;
                self.pos -= 1;
                self.peeked = PEEKED_NULL;
                Ok(self.peeked)
            }
            0x7b => {
                self.peeked = PEEKED_BEGIN_OBJECT;
                Ok(self.peeked)
            }
            _ => {
                self.pos -= 1;
                let result = self.peek_keyword()?;
                if result != PEEKED_NONE {
                    return Ok(result);
                }
                let result = self.peek_number()?;
                if result != PEEKED_NONE {
                    return Ok(result);
                }
                if !self.is_literal(self.buffer[self.pos])? {
                    return Err(self.syntax_error("Expected value"));
                }
                self.check_lenient()?;
                self.peeked = PEEKED_UNQUOTED;
                Ok(self.peeked)
            }
        }
    }

    fn peek_keyword(&mut self) -> Result<u8> {
        let c = self.buffer[self.pos];
        let (keyword, keyword_upper, peeking): (&[u8], &[u8], u8) = if c == b't' as u16 || c == b'T' as u16 {
            (b"true", b"TRUE", PEEKED_TRUE)
        } else if c == b'f' as u16 || c == b'F' as u16 {
            (b"false", b"FALSE", PEEKED_FALSE)
        } else if c == b'n' as u16 || c == b'N' as u16 {
            (b"null", b"NULL", PEEKED_NULL)
        } else {
            return Ok(PEEKED_NONE);
        };
        let allows_upper_cased = self.strictness != Strictness::Strict;
        let length = keyword.len();
        for i in 0..length {
            if self.pos + i >= self.limit() {
                return Ok(PEEKED_NONE);
            }
            let c = self.buffer[self.pos + i];
            let matched = c == keyword[i] as u16 || allows_upper_cased && c == keyword_upper[i] as u16;
            if !matched {
                return Ok(PEEKED_NONE);
            }
        }
        if self.pos + length < self.limit() && self.is_literal(self.buffer[self.pos + length])? {
            return Ok(PEEKED_NONE);
        }
        self.pos += length;
        self.peeked = peeking;
        Ok(self.peeked)
    }

    fn peek_number(&mut self) -> Result<u8> {
        let p = self.pos;
        let l = self.limit();
        let mut value: i64 = 0;
        let mut negative = false;
        let mut fits_in_long = true;
        let mut last = NUMBER_CHAR_NONE;
        let mut i = 0;
        loop {
            if i == BUFFER_LENGTH {
                return Ok(PEEKED_NONE);
            }
            if p + i == l {
                break;
            }
            let c = self.buffer[p + i];
            match c {
                0x2b => {
                    if last != NUMBER_CHAR_EXP_E {
                        return Ok(PEEKED_NONE);
                    }
                    last = NUMBER_CHAR_EXP_SIGN;
                }
                0x2d => {
                    if last == NUMBER_CHAR_NONE {
                        negative = true;
                        last = NUMBER_CHAR_SIGN;
                    } else if last == NUMBER_CHAR_EXP_E {
                        last = NUMBER_CHAR_EXP_SIGN;
                    } else {
                        return Ok(PEEKED_NONE);
                    }
                }
                0x2e => {
                    if last != NUMBER_CHAR_DIGIT {
                        return Ok(PEEKED_NONE);
                    }
                    last = NUMBER_CHAR_DECIMAL;
                }
                0x45 | 0x65 => {
                    if last != NUMBER_CHAR_DIGIT && last != NUMBER_CHAR_FRACTION_DIGIT {
                        return Ok(PEEKED_NONE);
                    }
                    last = NUMBER_CHAR_EXP_E;
                }
                _ => {
                    if !(0x30..=0x39).contains(&c) {
                        if self.is_literal(c)? {
                            return Ok(PEEKED_NONE);
                        }
                        break;
                    }
                    let digit = (c - 0x30) as i64;
                    if last == NUMBER_CHAR_SIGN || last == NUMBER_CHAR_NONE {
                        value = -digit;
                        last = NUMBER_CHAR_DIGIT;
                    } else if last == NUMBER_CHAR_DIGIT {
                        if value == 0 {
                            return Ok(PEEKED_NONE);
                        }
                        let new_value = value.wrapping_mul(10).wrapping_sub(digit);
                        fits_in_long &= value > MIN_INCOMPLETE_INTEGER || value == MIN_INCOMPLETE_INTEGER && new_value < value;
                        value = new_value;
                    } else if last == NUMBER_CHAR_DECIMAL {
                        last = NUMBER_CHAR_FRACTION_DIGIT;
                    } else if last == NUMBER_CHAR_EXP_E || last == NUMBER_CHAR_EXP_SIGN {
                        last = NUMBER_CHAR_EXP_DIGIT;
                    }
                }
            }
            i += 1;
        }
        if last == NUMBER_CHAR_DIGIT && fits_in_long && (value != i64::MIN || negative) && (value != 0 || !negative) {
            self.peeked_long = if negative { value } else { value.wrapping_neg() };
            self.pos += i;
            self.peeked = PEEKED_LONG;
            Ok(self.peeked)
        } else if last == NUMBER_CHAR_DIGIT || last == NUMBER_CHAR_FRACTION_DIGIT || last == NUMBER_CHAR_EXP_DIGIT {
            self.peeked_number_length = i;
            self.peeked = PEEKED_NUMBER;
            Ok(self.peeked)
        } else {
            Ok(PEEKED_NONE)
        }
    }

    fn is_literal(&self, c: u16) -> Result<bool> {
        match c {
            0x23 | 0x2f | 0x3b | 0x3d | 0x5c => {
                self.check_lenient()?;
                Ok(false)
            }
            0x09 | 0x0a | 0x0c | 0x0d | 0x20 | 0x2c | 0x3a | 0x5b | 0x5d | 0x7b | 0x7d => Ok(false),
            _ => Ok(true),
        }
    }

    pub fn next_name(&mut self) -> Result<String> {
        let result = match self.peeked_or_peek()? {
            PEEKED_UNQUOTED_NAME => self.next_unquoted_value()?,
            PEEKED_SINGLE_QUOTED_NAME => self.next_quoted_value(b'\'' as u16)?,
            PEEKED_DOUBLE_QUOTED_NAME => self.next_quoted_value(b'"' as u16)?,
            _ => return Err(self.unexpected_token_error("a name")?),
        };
        self.peeked = PEEKED_NONE;
        let index = self.stack.len() - 1;
        self.path_names[index] = Some(result.clone());
        Ok(result)
    }

    pub fn next_string(&mut self) -> Result<String> {
        let result = match self.peeked_or_peek()? {
            PEEKED_UNQUOTED => self.next_unquoted_value()?,
            PEEKED_SINGLE_QUOTED => self.next_quoted_value(b'\'' as u16)?,
            PEEKED_DOUBLE_QUOTED => self.next_quoted_value(b'"' as u16)?,
            PEEKED_BUFFERED => self.peeked_string.take().unwrap_or_default(),
            PEEKED_LONG => self.peeked_long.to_string(),
            PEEKED_NUMBER => {
                let result = String::from_utf16_lossy(&self.buffer[self.pos..self.pos + self.peeked_number_length]);
                self.pos += self.peeked_number_length;
                result
            }
            _ => return Err(self.unexpected_token_error("a string")?),
        };
        self.peeked = PEEKED_NONE;
        *self.top_index() += 1;
        Ok(result)
    }

    pub fn next_boolean(&mut self) -> Result<bool> {
        let result = match self.peeked_or_peek()? {
            PEEKED_TRUE => true,
            PEEKED_FALSE => false,
            _ => return Err(self.unexpected_token_error("a boolean")?),
        };
        self.peeked = PEEKED_NONE;
        *self.top_index() += 1;
        Ok(result)
    }

    pub fn next_null(&mut self) -> Result<()> {
        if self.peeked_or_peek()? != PEEKED_NULL {
            return Err(self.unexpected_token_error("null")?);
        }
        self.peeked = PEEKED_NONE;
        *self.top_index() += 1;
        Ok(())
    }

    fn next_quoted_value(&mut self, quote: u16) -> Result<String> {
        let mut builder: Vec<u16> = Vec::new();
        let mut start = self.pos;
        let mut p = self.pos;
        while p < self.limit() {
            let c = self.buffer[p];
            p += 1;
            if self.strictness == Strictness::Strict && c < 32 {
                return Err(self.syntax_error("Unescaped control characters (\\u0000-\\u001F) are not allowed in strict mode"));
            }
            if c == quote {
                self.pos = p;
                builder.extend_from_slice(&self.buffer[start..p - 1]);
                return Ok(String::from_utf16_lossy(&builder));
            }
            if c == 0x5c {
                self.pos = p;
                builder.extend_from_slice(&self.buffer[start..p - 1]);
                let escaped = self.read_escape_character()?;
                builder.push(escaped);
                p = self.pos;
                start = p;
            } else if c == 0x0a {
                self.line_number += 1;
                self.line_start = p;
            }
        }
        self.pos = p;
        Err(self.syntax_error("Unterminated string"))
    }

    fn next_unquoted_value(&mut self) -> Result<String> {
        let mut i = 0;
        while self.pos + i < self.limit() {
            match self.buffer[self.pos + i] {
                0x09 | 0x0a | 0x0c | 0x0d | 0x20 | 0x2c | 0x3a | 0x5b | 0x5d | 0x7b | 0x7d => break,
                0x23 | 0x2f | 0x3b | 0x3d | 0x5c => {
                    self.check_lenient()?;
                    break;
                }
                _ => i += 1,
            }
        }
        let result = String::from_utf16_lossy(&self.buffer[self.pos..self.pos + i]);
        self.pos += i;
        Ok(result)
    }

    fn push(&mut self, new_top: u8) -> Result<()> {
        if self.stack.len() - 1 >= self.nesting_limit {
            return Err(JsonReaderError::Malformed(format!("Nesting limit {} reached{}", self.nesting_limit, self.location_string())));
        }
        self.stack.push(new_top);
        self.path_names.push(None);
        self.path_indices.push(0);
        Ok(())
    }

    fn pop(&mut self) {
        self.stack.pop();
        self.path_names.pop();
        self.path_indices.pop();
    }

    fn next_non_whitespace(&mut self, throw_on_eof: bool) -> Result<i32> {
        let mut p = self.pos;
        loop {
            if p == self.limit() {
                self.pos = p;
                if throw_on_eof {
                    return Err(JsonReaderError::EndOfInput(format!("End of input{}", self.location_string())));
                }
                return Ok(-1);
            }
            let c = self.buffer[p];
            p += 1;
            if c == 0x0a {
                self.line_number += 1;
                self.line_start = p;
            } else if c != 0x20 && c != 0x0d && c != 0x09 {
                if c == 0x2f {
                    self.pos = p;
                    if p == self.limit() {
                        return Ok(c as i32);
                    }
                    self.check_lenient()?;
                    match self.buffer[self.pos] {
                        0x2a => {
                            self.pos += 1;
                            if !self.skip_to(b"*/") {
                                return Err(self.syntax_error("Unterminated comment"));
                            }
                            p = self.pos + 2;
                        }
                        0x2f => {
                            self.pos += 1;
                            self.skip_to_end_of_line();
                            p = self.pos;
                        }
                        _ => return Ok(c as i32),
                    }
                } else if c == 0x23 {
                    self.pos = p;
                    self.check_lenient()?;
                    self.skip_to_end_of_line();
                    p = self.pos;
                } else {
                    self.pos = p;
                    return Ok(c as i32);
                }
            }
        }
    }

    fn check_lenient(&self) -> Result<()> {
        if self.strictness != Strictness::Lenient {
            return Err(self.syntax_error("Use JsonReader.setStrictness(Strictness.LENIENT) to accept malformed JSON"));
        }
        Ok(())
    }

    fn skip_to_end_of_line(&mut self) {
        while self.pos < self.limit() {
            let c = self.buffer[self.pos];
            self.pos += 1;
            if c == 0x0a {
                self.line_number += 1;
                self.line_start = self.pos;
                break;
            } else if c == 0x0d {
                break;
            }
        }
    }

    fn skip_to(&mut self, to_find: &[u8]) -> bool {
        let length = to_find.len();
        while self.pos + length <= self.limit() {
            if self.buffer[self.pos] == 0x0a {
                self.line_number += 1;
                self.line_start = self.pos + 1;
            } else if to_find.iter().enumerate().all(|(c, expected)| self.buffer[self.pos + c] == *expected as u16) {
                return true;
            }
            self.pos += 1;
        }
        false
    }

    fn location_string(&self) -> String {
        let line = self.line_number + 1;
        let column = self.pos as i64 - self.line_start as i64 + 1;
        format!(" at line {line} column {column} path {}", self.get_path())
    }

    pub fn get_path(&self) -> String {
        self.get_path_with(false)
    }

    pub fn get_previous_path(&self) -> String {
        self.get_path_with(true)
    }

    fn get_path_with(&self, use_previous_path: bool) -> String {
        let mut result = String::from("$");
        for (i, scope) in self.stack.iter().enumerate() {
            match *scope {
                EMPTY_ARRAY | NONEMPTY_ARRAY => {
                    let mut path_index = self.path_indices[i];
                    if use_previous_path && path_index > 0 && i == self.stack.len() - 1 {
                        path_index -= 1;
                    }
                    result.push('[');
                    result.push_str(&path_index.to_string());
                    result.push(']');
                }
                EMPTY_OBJECT | DANGLING_NAME | NONEMPTY_OBJECT => {
                    result.push('.');
                    if let Some(name) = &self.path_names[i] {
                        result.push_str(name);
                    }
                }
                _ => {}
            }
        }
        result
    }

    fn read_escape_character(&mut self) -> Result<u16> {
        if self.pos == self.limit() {
            return Err(self.syntax_error("Unterminated escape sequence"));
        }
        let escaped = self.buffer[self.pos];
        self.pos += 1;
        match escaped {
            0x0a | 0x27 => {
                if escaped == 0x0a {
                    if self.strictness == Strictness::Strict {
                        return Err(self.syntax_error("Cannot escape a newline character in strict mode"));
                    }
                    self.line_number += 1;
                    self.line_start = self.pos;
                }
                if self.strictness == Strictness::Strict {
                    return Err(self.syntax_error("Invalid escaped character \"'\" in strict mode"));
                }
                Ok(escaped)
            }
            0x22 | 0x2f | 0x5c => Ok(escaped),
            0x62 => Ok(0x08),
            0x66 => Ok(0x0c),
            0x6e => Ok(0x0a),
            0x72 => Ok(0x0d),
            0x74 => Ok(0x09),
            0x75 => {
                if self.pos + 4 > self.limit() {
                    return Err(self.syntax_error("Unterminated escape sequence"));
                }
                let mut result: u16 = 0;
                for i in self.pos..self.pos + 4 {
                    let c = self.buffer[i];
                    result <<= 4;
                    result += match c {
                        0x30..=0x39 => c - 0x30,
                        0x61..=0x66 => c - 0x61 + 10,
                        0x41..=0x46 => c - 0x41 + 10,
                        _ => {
                            let text = String::from_utf16_lossy(&self.buffer[self.pos..self.pos + 4]);
                            return Err(self.syntax_error(&format!("Malformed Unicode escape \\u{text}")));
                        }
                    };
                }
                self.pos += 4;
                Ok(result)
            }
            _ => Err(self.syntax_error("Invalid escape sequence")),
        }
    }

    fn syntax_error(&self, message: &str) -> JsonReaderError {
        JsonReaderError::Malformed(format!("{message}{}\nSee https://github.com/google/gson/blob/main/Troubleshooting.md#malformed-json", self.location_string()))
    }

    fn unexpected_token_error(&mut self, expected: &str) -> Result<JsonReaderError> {
        let peeked = self.peek()?;
        let troubleshooting_id = if peeked == JsonToken::Null { "adapter-not-null-safe" } else { "unexpected-json-structure" };
        Ok(JsonReaderError::IllegalState(format!(
            "Expected {expected} but was {}{}\nSee https://github.com/google/gson/blob/main/Troubleshooting.md#{troubleshooting_id}",
            json_token_name(peeked),
            self.location_string()
        )))
    }

    fn consume_non_execute_prefix(&mut self) -> Result<()> {
        self.next_non_whitespace(true)?;
        self.pos -= 1;
        if self.pos + 5 <= self.limit() {
            let p = self.pos;
            if self.buffer[p..p + 5] == [0x29, 0x5d, 0x7d, 0x27, 0x0a] {
                self.pos += 5;
            }
        }
        Ok(())
    }
}

fn json_token_name(token: JsonToken) -> &'static str {
    match token {
        JsonToken::BeginArray => "BEGIN_ARRAY",
        JsonToken::EndArray => "END_ARRAY",
        JsonToken::BeginObject => "BEGIN_OBJECT",
        JsonToken::EndObject => "END_OBJECT",
        JsonToken::Name => "NAME",
        JsonToken::String => "STRING",
        JsonToken::Number => "NUMBER",
        JsonToken::Boolean => "BOOLEAN",
        JsonToken::Null => "NULL",
        JsonToken::EndDocument => "END_DOCUMENT",
    }
}
