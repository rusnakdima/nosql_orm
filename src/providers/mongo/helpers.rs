use serde_json::Value;

pub fn normalize_id(mut v: Value) -> Value {
  if let Some(obj) = v.as_object_mut() {
    if let Some(id) = obj.remove("_id") {
      obj.insert("id".to_string(), id);
    }
  }
  v
}

pub fn regex_escape(s: &str) -> String {
  s.chars()
    .flat_map(|c| {
      if "^$.*+?()[]{}|\\".contains(c) {
        vec!['\\', c]
      } else {
        vec![c]
      }
    })
    .collect()
}
