use std::io::{self, Write};

const LINE_SEPARATOR: &str = "\r\n";
const FIELD_SEPARATOR: &str = ",";

pub struct CsvOutput<W: Write> {
    output: W,
    column_count: usize,
}

pub struct Builder {
    headers: Vec<String>,
}

impl Builder {
    pub fn add_column(mut self, header: &str) -> Builder {
        self.headers.push(header.to_string());
        self
    }

    pub fn build<W: Write>(self, writer: W) -> io::Result<CsvOutput<W>> {
        let mut output = CsvOutput { output: writer, column_count: self.headers.len() };
        output.write_line(&self.headers)?;
        Ok(output)
    }
}

pub fn escape_csv(value: &str) -> String {
    if value.contains([',', '"', '\r', '\n']) { format!("\"{}\"", value.replace('"', "\"\"")) } else { value.to_string() }
}

pub fn builder() -> Builder {
    Builder { headers: Vec::new() }
}

impl<W: Write> CsvOutput<W> {
    pub fn write_row(&mut self, values: &[Option<String>]) -> io::Result<()> {
        if values.len() != self.column_count {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, format!("Invalid number of columns, expected {}, but got {}", self.column_count, values.len())));
        }
        let values: Vec<String> = values.iter().map(|value| value.clone().unwrap_or_else(|| "[null]".to_string())).collect();
        self.write_line(&values)
    }

    fn write_line(&mut self, values: &[String]) -> io::Result<()> {
        let line = values.iter().map(|value| escape_csv(value)).collect::<Vec<_>>().join(FIELD_SEPARATOR);
        self.output.write_all(line.as_bytes())?;
        self.output.write_all(LINE_SEPARATOR.as_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_use_crlf_quote_special_fields_and_print_nulls() {
        let mut bytes = Vec::new();
        let mut csv_output = builder().add_column("a").add_column("b,c").build(&mut bytes).unwrap();
        csv_output.write_row(&[Some("say \"hi\"".to_string()), None]).unwrap();
        assert!(csv_output.write_row(&[None]).is_err());
        assert_eq!(String::from_utf8(bytes).unwrap(), "a,\"b,c\"\r\n\"say \"\"hi\"\"\",[null]\r\n");
    }
}
