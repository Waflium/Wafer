use wafer_core::java_string::JavaStr;
use crate::commands::brigadier::{CommandSyntaxException, StringReader, tr};

#[derive(Clone, Debug)]
pub struct CompilableString<T> {
    source: String,
    compiled: T,
}

impl<T> CompilableString<T> {
    pub fn source(&self) -> &str {
        &self.source
    }

    pub fn compiled(&self) -> &T {
        &self.compiled
    }

    pub fn into_compiled(self) -> T {
        self.compiled
    }
}

impl<T> PartialEq for CompilableString<T> {
    fn eq(&self, other: &CompilableString<T>) -> bool {
        self.source == other.source
    }
}

pub fn command_parser_helper<T>(
    contents: &str,
    parse: impl FnOnce(&mut StringReader) -> Result<T, CommandSyntaxException>,
    error_message: impl FnOnce(&str, &CommandSyntaxException) -> String,
) -> Result<CompilableString<T>, String> {
    let units: Vec<u16> = contents.java_utf16().collect();
    let mut reader = StringReader::new(&units);
    let result = parse(&mut reader).and_then(|result| {
        if reader.can_read() {
            let parsed = reader.substring(0, reader.cursor);
            let leftovers = reader.substring(reader.cursor, reader.total_length());
            return Err(CommandSyntaxException::new(tr("command.trailing_data", vec![format!("{parsed}[{leftovers}]").into()])));
        }
        Ok(result)
    });
    match result {
        Ok(compiled) => Ok(CompilableString { source: contents.to_string(), compiled }),
        Err(exception) => Err(error_message(contents, &exception)),
    }
}
