use std::fmt::Write;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JavaThrowable {
    pub description: String,
    pub stack_trace: Vec<&'static str>,
    pub cause: Option<Box<JavaThrowable>>,
}

impl JavaThrowable {
    pub fn new(description: impl Into<String>, stack_trace: Vec<&'static str>) -> JavaThrowable {
        JavaThrowable { description: description.into(), stack_trace, cause: None }
    }

    pub fn with_cause(mut self, cause: JavaThrowable) -> JavaThrowable {
        self.cause = Some(Box::new(cause));
        self
    }

    pub fn print_stack_trace(&self) -> String {
        let mut out = String::new();
        out.push_str(&self.description);
        out.push('\n');
        for frame in &self.stack_trace {
            let _ = writeln!(out, "\tat {frame}");
        }
        let mut enclosing = self;
        while let Some(cause) = enclosing.cause.as_deref() {
            print_enclosed_stack_trace(cause, &enclosing.stack_trace, &mut out);
            enclosing = cause;
        }
        out
    }
}

impl std::fmt::Display for JavaThrowable {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.description)
    }
}

impl std::error::Error for JavaThrowable {}

fn print_enclosed_stack_trace(throwable: &JavaThrowable, enclosing_trace: &[&'static str], out: &mut String) {
    let trace = &throwable.stack_trace;
    let mut m = trace.len() as isize - 1;
    let mut n = enclosing_trace.len() as isize - 1;
    while m >= 0 && n >= 0 && trace[m as usize] == enclosing_trace[n as usize] {
        m -= 1;
        n -= 1;
    }
    let frames_in_common = trace.len() as isize - 1 - m;
    out.push_str("Caused by: ");
    out.push_str(&throwable.description);
    out.push('\n');
    for frame in &trace[..(m + 1) as usize] {
        let _ = writeln!(out, "\tat {frame}");
    }
    if frames_in_common != 0 {
        let _ = writeln!(out, "\t... {frames_in_common} more");
    }
}

#[cfg(test)]
mod tests;
