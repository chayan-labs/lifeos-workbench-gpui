//! Output rendering: machine-readable JSON mode vs a compact human summary.

use serde_json::Value;

#[derive(Clone, Copy)]
pub struct Output {
    pub json: bool,
}

impl Output {
    /// Render a successful result. In JSON mode the raw value is printed
    /// verbatim; otherwise a `summary` line plus a readable body.
    pub fn ok(&self, summary: &str, value: &Value) {
        if self.json {
            println!("{}", serde_json::to_string_pretty(value).unwrap_or_default());
            return;
        }
        if !summary.is_empty() {
            println!("{summary}");
        }
        match value {
            Value::Null => {}
            Value::Array(items) => render_array(items),
            _ => println!("{}", serde_json::to_string_pretty(value).unwrap_or_default()),
        }
    }

}

fn render_array(items: &[Value]) {
    if items.is_empty() {
        println!("(none)");
        return;
    }
    for (i, item) in items.iter().enumerate() {
        if i > 0 {
            println!("{}", "-".repeat(50));
        }
        match item {
            Value::Object(_) => {
                let id = item.get("id").and_then(Value::as_str).unwrap_or("?");
                let title = item.get("title").and_then(Value::as_str);
                match title {
                    Some(t) => println!("{id}  {t}"),
                    None => println!("{id}"),
                }
                println!("{}", serde_json::to_string_pretty(item).unwrap_or_default());
            }
            _ => println!("{item}"),
        }
    }
}
