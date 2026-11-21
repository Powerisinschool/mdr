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
    #[allow(dead_code)]
    List(Vec<Value>), // TODO: Implement List variant for future use cases
    #[allow(dead_code)]
    Object(BTreeMap<String, Value>), // TODO: Implement Object variant for future use cases
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_nested_paths_and_tuple_indices() {
        let mut author = BTreeMap::new();
        author.insert("name".to_string(), Value::Text("Tolu".to_string()));

        let mut root = BTreeMap::new();
        root.insert("author".to_string(), Value::Object(author));
        root.insert(
            "pair".to_string(),
            Value::Tuple(vec![
                Value::Text("Rust".to_string()),
                Value::Text("Systems".to_string()),
            ]),
        );

        let val = Value::Object(root);

        assert_eq!(
            val.resolve_path("author.name"),
            Some(&Value::Text("Tolu".to_string()))
        );
        assert_eq!(
            val.resolve_path("pair.1"),
            Some(&Value::Text("Systems".to_string()))
        );

        assert_eq!(val.resolve_path("pair.5"), None);
        assert_eq!(val.resolve_path("author.name.extra"), None);
    }

    #[test]
    fn resolves_text_and_theme_variants() {
        let theme = Value::Theme(ThemeValue {
            dark: "14B8A6".to_string(),
            light: "0F766E".to_string(),
        });
        let tuple = Value::Tuple(vec![Value::Text("Badge".to_string()), theme.clone()]);

        assert_eq!(theme.resolve_text(""), Some("14B8A6"));
        assert_eq!(theme.resolve_text("dark"), Some("14B8A6"));
        assert_eq!(theme.resolve_text("light"), Some("0F766E"));
        assert_eq!(theme.resolve_text("invalid"), None);

        assert_eq!(tuple.resolve_text(""), None);
        assert_eq!(tuple.resolve_text("0"), Some("Badge"));
        assert_eq!(tuple.resolve_text("1"), Some("14B8A6"));
        assert_eq!(tuple.resolve_text("1.light"), Some("0F766E"));
    }
}
