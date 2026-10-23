use super::JsonElement;

pub fn write_value(out: &mut String, value: &JsonElement, sort_keys: bool) {
    match value {
        JsonElement::Null => out.push_str("null"),
        JsonElement::Boolean(value) => out.push_str(if *value { "true" } else { "false" }),
        JsonElement::Number(value) => out.push_str(value),
        JsonElement::String(value) => string(out, value),
        JsonElement::Array(elements) => {
            out.push('[');
            for (i, element) in elements.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_value(out, element, sort_keys);
            }
            out.push(']');
        }
        JsonElement::Object(members) => {
            out.push('{');
            let mut entries: Vec<&(String, JsonElement)> = members.iter().collect();
            if sort_keys {
                entries.sort_by(|(a, _), (b, _)| a.encode_utf16().cmp(b.encode_utf16()));
            }
            for (i, (name, element)) in entries.into_iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                string(out, name);
                out.push(':');
                write_value(out, element, sort_keys);
            }
            out.push('}');
        }
    }
}

fn string(out: &mut String, value: &str) {
    out.push('"');
    for c in value.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\u{c}' => out.push_str("\\f"),
            '\u{0}'..='\u{1f}' => {
                out.push_str("\\u00");
                out.push(char::from_digit(c as u32 >> 4, 16).unwrap());
                out.push(char::from_digit(c as u32 & 0xf, 16).unwrap());
            }
            '\u{2028}' => out.push_str("\\u2028"),
            '\u{2029}' => out.push_str("\\u2029"),
            _ => out.push(c),
        }
    }
    out.push('"');
}
