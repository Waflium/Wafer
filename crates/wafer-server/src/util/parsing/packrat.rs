use crate::commands::brigadier::{
    CommandSyntaxException, DISPATCHER_PARSE_EXCEPTION, DynamicCommandExceptionType, LITERAL_INCORRECT, SimpleCommandExceptionType, StringReader,
};

#[derive(Clone)]
pub enum Delayed {
    Simple(&'static SimpleCommandExceptionType),
    Dynamic(&'static DynamicCommandExceptionType, String),
}

impl Delayed {
    fn create(&self, input: &[u16], cursor: usize) -> CommandSyntaxException {
        let mut reader = StringReader::new(input);
        reader.cursor = cursor;
        match self {
            Delayed::Simple(e) => e.create_with_context(&reader),
            Delayed::Dynamic(e, arg) => e.create_with_context(&reader, arg.as_str()),
        }
    }
}

#[derive(Clone)]
pub enum Reason {
    Delayed(Delayed),
    Exception(CommandSyntaxException),
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ResourceKind {
    Element,
    Tag,
    Component,
    Predicate,
}

#[derive(Clone)]
pub enum SuggestionSupplier {
    Empty,
    Characters(Vec<char>),
    Resources(ResourceKind),
}

struct ErrorEntry {
    suggestions: SuggestionSupplier,
    reason: Reason,
}

#[derive(Default)]
struct ErrorCollector {
    entries: Vec<ErrorEntry>,
    last_cursor: Option<usize>,
}

impl ErrorCollector {
    fn discard_errors_from_shorter_parse(&mut self, cursor: usize) {
        if self.last_cursor.is_none_or(|last| cursor > last) {
            self.last_cursor = Some(cursor);
            self.entries.clear();
        }
    }

    fn store(&mut self, cursor: usize, suggestions: SuggestionSupplier, reason: Reason) {
        self.discard_errors_from_shorter_parse(cursor);
        if self.last_cursor == Some(cursor) {
            self.entries.push(ErrorEntry { suggestions, reason });
        }
    }
}

pub struct ParseState<'r, 'a> {
    pub input: &'r mut StringReader<'a>,
    errors: ErrorCollector,
    silent: u32,
    for_suggestions: bool,
}

impl<'r, 'a> ParseState<'r, 'a> {
    pub fn new(input: &'r mut StringReader<'a>) -> ParseState<'r, 'a> {
        ParseState { input, errors: ErrorCollector::default(), silent: 0, for_suggestions: false }
    }

    pub fn for_suggestions(input: &'r mut StringReader<'a>) -> ParseState<'r, 'a> {
        ParseState { input, errors: ErrorCollector::default(), silent: 0, for_suggestions: true }
    }

    pub fn mark(&self) -> usize {
        self.input.cursor
    }

    pub fn restore(&mut self, mark: usize) {
        self.input.cursor = mark;
    }

    pub fn store(&mut self, cursor: usize, reason: Reason) {
        self.store_suggesting(cursor, SuggestionSupplier::Empty, reason);
    }

    pub fn store_suggesting(&mut self, cursor: usize, suggestions: SuggestionSupplier, reason: Reason) {
        if self.silent == 0 {
            self.errors.store(cursor, suggestions, reason);
        }
    }

    pub fn store_delayed(&mut self, cursor: usize, delayed: Delayed) {
        self.store(cursor, Reason::Delayed(delayed));
    }

    pub fn lookahead(&mut self, parse: impl FnOnce(&mut Self) -> bool) -> bool {
        let mark = self.mark();
        self.silent += 1;
        let result = parse(self);
        self.silent -= 1;
        self.restore(mark);
        result
    }

    pub fn characters(&mut self, values: &[char]) -> bool {
        self.input.skip_whitespace();
        let cursor = self.mark();
        if self.input.can_read() {
            let c = self.input.read();
            if values.iter().any(|&v| v as u32 == c as u32) {
                return true;
            }
        }
        let joined = values.iter().map(|c| c.to_string()).collect::<Vec<_>>().join("|");
        let suggestions = if self.for_suggestions { SuggestionSupplier::Characters(values.to_vec()) } else { SuggestionSupplier::Empty };
        self.store_suggesting(cursor, suggestions, Reason::Delayed(Delayed::Dynamic(&LITERAL_INCORRECT, joined)));
        false
    }

    pub fn character(&mut self, value: char) -> bool {
        self.characters(&[value])
    }

    pub fn finish<T>(self, result: Option<T>) -> Result<T, CommandSyntaxException> {
        if let Some(result) = result {
            return Ok(result);
        }
        let cursor = self.errors.last_cursor.unwrap_or(0);
        let input = self.input.string();
        match self.errors.entries.into_iter().next().map(|entry| entry.reason) {
            Some(Reason::Delayed(delayed)) => Err(delayed.create(input, cursor)),
            Some(Reason::Exception(e)) => Err(e),
            None => Err(DISPATCHER_PARSE_EXCEPTION.create("Failed to parse")),
        }
    }

    pub fn finish_suggestions<T>(mut self, result: &Option<T>) -> (usize, Vec<SuggestionSupplier>) {
        if result.is_some() {
            let mark = self.mark();
            self.errors.discard_errors_from_shorter_parse(mark);
        }
        let cursor = self.errors.last_cursor.unwrap_or(0);
        (cursor, self.errors.entries.into_iter().map(|entry| entry.suggestions).collect())
    }
}
