use wafer_core::java_string::JavaStr;
use serde_json::Value;

pub fn parse_string(json: &str) -> Result<Value, String> {
    crate::util::gson::parse_lenient_value(json).map_err(|error| error.split("\nCaused by: ").next().unwrap_or_default().to_string())
}

pub fn type_name(element: Option<&Value>) -> String {
    let Some(element) = element else { return "null (missing)".into() };
    let text = element.to_string();
    let units: Vec<u16> = text.java_utf16().collect();
    let value = if units.len() <= 10 { text.clone() } else { format!("{}...{}", wafer_core::java_string::from_utf16(&units[..4]), wafer_core::java_string::from_utf16(&units[units.len() - 3..])) };
    match element {
        Value::Null => "null (json)".into(),
        Value::Array(_) => format!("an array ({value})"),
        Value::Object(_) => format!("an object ({value})"),
        Value::Number(_) => format!("a number ({value})"),
        Value::Bool(_) => format!("a boolean ({value})"),
        Value::String(_) => value,
    }
}

pub fn as_string(element: &Value) -> Option<String> {
    match element {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}
