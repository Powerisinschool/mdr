use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemeValue {
    pub dark: String,
    pub light: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    Theme(ThemeValue),
    Text(String),
    Tuple(Vec<Value>),
    List(Vec<Value>),
    Object(BTreeMap<String, Value>),
}

impl Value {
    pub fn resolve_path(&self, path: &str) -> Option<&Value> {
        if path.is_empty() {
            return Some(self);
        }

        let mut current_value = self;
        for segment in path.split('.') {
            match current_value {
                Value::Object(map) => current_value = map.get(segment)?,
                Value::Tuple(items) | Value::List(items) => {
                    current_value = items.get(segment.parse::<usize>().ok()?)?
                }
                _ => return None,
            }
        }

        Some(current_value)
    }

    pub fn resolve_text(&self, path: &str) -> Option<&str> {
        if path.is_empty() {
            return match self {
                Value::Text(s) => Some(s.as_str()),
                Value::Theme(theme) => Some(theme.dark.as_str()),
                _ => None,
            };
        }

        let (target, last) = match path.rsplit_once('.') {
            Some((parent, last)) => (self.resolve_path(parent)?, last),
            None => (self, path),
        };

        match target {
            Value::Theme(theme) => match last {
                "dark" => Some(theme.dark.as_str()),
                "light" => Some(theme.light.as_str()),
                _ => None,
            },
            _ => match target.resolve_path(last)? {
                Value::Text(s) => Some(s.as_str()),
                Value::Theme(theme) => Some(theme.dark.as_str()),
                _ => None,
            },
        }
    }
}
