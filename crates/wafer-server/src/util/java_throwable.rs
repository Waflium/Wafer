use std::fmt::Write;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ThrowableReport {
    pub title: String,
    pub category: String,
    pub details: Vec<(String, String)>,
    pub category_stack_trace: Vec<&'static str>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JavaThrowable {
    pub description: String,
    pub stack_trace: Vec<&'static str>,
    pub cause: Option<Box<JavaThrowable>>,
    pub report: Option<ThrowableReport>,
}

impl JavaThrowable {
    pub fn new(description: impl Into<String>, stack_trace: Vec<&'static str>) -> JavaThrowable {
        JavaThrowable { description: description.into(), stack_trace, cause: None, report: None }
    }

    pub fn with_report(mut self, report: ThrowableReport) -> JavaThrowable {
        self.report = Some(report);
        self
    }

    pub fn push_frame(&mut self, frame: &'static str) {
        self.stack_trace.push(frame);
        if let Some(report) = &mut self.report {
            report.category_stack_trace.push(frame);
        }
        if let Some(cause) = self.cause.as_deref_mut() {
            cause.push_frame(frame);
        }
    }

    pub fn push_frames(&mut self, frames: &[&'static str]) {
        for frame in frames {
            self.push_frame(frame);
        }
    }

    pub fn with_frame(mut self, frame: &'static str) -> JavaThrowable {
        self.stack_trace.push(frame);
        self
    }

    pub fn with_cause(mut self, cause: JavaThrowable) -> JavaThrowable {
        self.cause = Some(Box::new(cause));
        self
    }

    pub fn without_stack_trace(mut self) -> JavaThrowable {
        self.stack_trace.clear();
        self.cause = self.cause.take().map(|cause| Box::new(cause.without_stack_trace()));
        self
    }

    pub fn from_io_error(error: &std::io::Error) -> JavaThrowable {
        match error.get_ref() {
            Some(inner) => match inner.downcast_ref::<JavaThrowable>() {
                Some(throwable) => throwable.clone(),
                None => match inner.downcast_ref::<crate::util::java_nio::JavaException>() {
                    Some(exception) => JavaThrowable::new(exception.to_string(), Vec::new()),
                    None => JavaThrowable::new(format!("java.io.IOException: {inner}"), Vec::new()),
                },
            },
            None => JavaThrowable::new(format!("java.io.IOException: {error}"), Vec::new()),
        }
    }

    pub fn is_io_exception(&self) -> bool {
        let class_name = self.class_name();
        matches!(class_name, "java.io.IOException" | "java.io.EOFException" | "java.io.UTFDataFormatException" | "java.io.FileNotFoundException" | "java.util.zip.ZipException")
            || (class_name.starts_with("java.nio.file.") && class_name.ends_with("Exception"))
    }

    pub fn class_name(&self) -> &str {
        self.description.split_once(": ").map_or(self.description.as_str(), |(class_name, _)| class_name)
    }

    pub fn message(&self) -> Option<&str> {
        self.description.split_once(": ").map(|(_, message)| message)
    }

    pub fn with_frames(mut self, frames: &[&'static str]) -> JavaThrowable {
        self.stack_trace.extend_from_slice(frames);
        self
    }

    pub fn print_stack_trace(&self) -> String {
        self.print_stack_trace_with_separator("\n")
    }

    pub fn print_stack_trace_with_separator(&self, separator: &str) -> String {
        let mut out = String::new();
        out.push_str(&self.description);
        out.push_str(separator);
        for frame in &self.stack_trace {
            let _ = write!(out, "\tat {frame}{separator}");
        }
        let mut enclosing = self;
        while let Some(cause) = enclosing.cause.as_deref() {
            print_enclosed_stack_trace(cause, &enclosing.stack_trace, &mut out, separator);
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

fn print_enclosed_stack_trace(throwable: &JavaThrowable, enclosing_trace: &[&'static str], out: &mut String, separator: &str) {
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
    out.push_str(separator);
    for frame in &trace[..(m + 1) as usize] {
        let _ = write!(out, "\tat {frame}{separator}");
    }
    if frames_in_common != 0 {
        let _ = write!(out, "\t... {frames_in_common} more{separator}");
    }
}

#[cfg(test)]
mod tests;
