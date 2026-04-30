use super::ValidationType;
use quote::ToTokens;
use syn::Meta;

pub fn parse_validation_meta(meta: &Meta) -> Vec<ValidationType> {
  let mut validations = Vec::new();
  match meta {
    Meta::List(list) => {
      let tokens: Vec<_> = list.tokens.clone().into_iter().collect();
      let mut i = 0;
      while i < tokens.len() {
        let token_str = tokens[i].to_string();
        match token_str.as_str() {
          "email" => validations.push(ValidationType::Email),
          "uuid" => validations.push(ValidationType::Uuid),
          "url" => validations.push(ValidationType::Url),
          "not_empty" => validations.push(ValidationType::NotEmpty),
          "non_null" => validations.push(ValidationType::NonNull),
          "required" => validations.push(ValidationType::Required),
          "length" => {
            if i + 1 < tokens.len() {
              let (mn, mx) = parse_length_args(&tokens[i + 1].to_string());
              validations.push(ValidationType::Length(mn, mx));
              i += 1
            }
          }
          "min" => {
            if i + 1 < tokens.len() {
              if let Ok(x) = tokens[i + 1]
                .to_string()
                .trim()
                .trim_matches('(')
                .trim_matches(')')
                .parse::<f64>()
              {
                validations.push(ValidationType::Min(x));
              }
              i += 1
            }
          }
          "max" => {
            if i + 1 < tokens.len() {
              if let Ok(x) = tokens[i + 1]
                .to_string()
                .trim()
                .trim_matches('(')
                .trim_matches(')')
                .parse::<f64>()
              {
                validations.push(ValidationType::Max(x));
              }
              i += 1
            }
          }
          "pattern" => {
            if i + 1 < tokens.len() {
              if let Some(p) = parse_pattern_arg(&tokens[i + 1].to_string()) {
                validations.push(ValidationType::Pattern(p));
              }
              i += 1
            }
          }
          "range" => {
            if i + 1 < tokens.len() {
              let (mn, mx) = parse_range_args(&tokens[i + 1].to_string());
              validations.push(ValidationType::Range(mn, mx));
              i += 1
            }
          }
          _ => {}
        }
        i += 1;
      }
    }
    Meta::NameValue(nv) => {
      let ident_str = nv
        .path
        .get_ident()
        .map(|i| i.to_string())
        .unwrap_or_default();
      match ident_str.as_str() {
        "email" => validations.push(ValidationType::Email),
        "uuid" => validations.push(ValidationType::Uuid),
        "url" => validations.push(ValidationType::Url),
        "required" => validations.push(ValidationType::Required),
        "length" => {
          if let Ok(lit) = syn::parse2::<syn::LitStr>(nv.value.to_token_stream()) {
            let (mn, mx) = parse_length_args(&lit.value());
            validations.push(ValidationType::Length(mn, mx))
          }
        }
        "min" => {
          if let Ok(lit) = syn::parse2::<syn::LitStr>(nv.value.to_token_stream()) {
            if let Ok(x) = lit.value().parse::<f64>() {
              validations.push(ValidationType::Min(x))
            }
          }
        }
        "max" => {
          if let Ok(lit) = syn::parse2::<syn::LitStr>(nv.value.to_token_stream()) {
            if let Ok(x) = lit.value().parse::<f64>() {
              validations.push(ValidationType::Max(x))
            }
          }
        }
        "pattern" => {
          if let Ok(lit) = syn::parse2::<syn::LitStr>(nv.value.to_token_stream()) {
            validations.push(ValidationType::Pattern(lit.value()))
          }
        }
        "range" => {
          if let Ok(lit) = syn::parse2::<syn::LitStr>(nv.value.to_token_stream()) {
            let (mn, mx) = parse_range_args(&lit.value());
            validations.push(ValidationType::Range(mn, mx))
          }
        }
        _ => {}
      }
    }
    _ => {}
  }
  validations
}

pub fn parse_length_args(args: &str) -> (Option<usize>, Option<usize>) {
  let args = args.trim().trim_start_matches('(').trim_end_matches(')');
  let mut min = None;
  let mut max = None;
  for part in args.split(',') {
    let part = part.trim();
    if part.starts_with("min = ") {
      if let Ok(v) = part[6..].parse::<usize>() {
        min = Some(v);
      }
    } else if part.starts_with("max = ") {
      if let Ok(v) = part[6..].parse::<usize>() {
        max = Some(v);
      }
    }
  }
  (min, max)
}

pub fn parse_range_args(args: &str) -> (Option<f64>, Option<f64>) {
  let args = args.trim().trim_start_matches('(').trim_end_matches(')');
  let mut min = None;
  let mut max = None;
  for part in args.split(',') {
    let part = part.trim();
    if part.starts_with("min = ") {
      if let Ok(v) = part[6..].parse::<f64>() {
        min = Some(v);
      }
    } else if part.starts_with("max = ") {
      if let Ok(v) = part[6..].parse::<f64>() {
        max = Some(v);
      }
    }
  }
  (min, max)
}

pub fn parse_pattern_arg(args: &str) -> Option<String> {
  let trimmed = args.trim().trim_start_matches('(').trim_end_matches(')');
  if (trimmed.starts_with('"') && trimmed.ends_with('"'))
    || (trimmed.starts_with('\'') && trimmed.ends_with('\''))
  {
    Some(trimmed[1..trimmed.len() - 1].to_string())
  } else {
    Some(trimmed.to_string())
  }
}
