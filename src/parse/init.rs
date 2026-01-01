use half::bf16;

use super::args::{parse_type, TypeSpec};
use crate::sim::generate_arange;

#[derive(Clone, Debug)]
pub(super) enum Number {
  Int(i64),
  Float(f32),
}

pub(super) fn parse_initializer(
  value: &str,
  spec: &TypeSpec,
  expected_len: usize,
  _shape: &[usize],
) -> Result<Vec<Number>, String> {
  use rand::Rng;

  let value = value.trim();
  let numbers = if let Some(inner) = value.strip_prefix('[').and_then(|v| v.strip_suffix(']')) {
    parse_number_list(inner)?
  } else if let Some(args) = value.strip_prefix("repeat(").and_then(|v| v.strip_suffix(')')) {
    let args = parse_call_tokens(args);
    if args.len() != 1 {
      return Err("repeat expects 1 arg".to_string());
    }
    let value = parse_number(args[0])?;
    vec![value; expected_len]
  } else if let Some(args) = value.strip_prefix("arange(").and_then(|v| v.strip_suffix(')')) {
    let args = parse_call_tokens(args);
    let (start, end, step) = match args.len() {
      1 => (0, parse_i32_arg(args[0], "arange end")?, 1),
      2 => (
        parse_i32_arg(args[0], "arange start")?,
        parse_i32_arg(args[1], "arange end")?,
        1,
      ),
      3 => (
        parse_i32_arg(args[0], "arange start")?,
        parse_i32_arg(args[1], "arange end")?,
        parse_i32_arg(args[2], "arange step")?,
      ),
      _ => return Err("arange expects 1-3 args".to_string()),
    };
    generate_arange(start, end, step)?
      .into_iter()
      .map(|val| Number::Int(val as i64))
      .collect()
  } else if value.strip_prefix("rand(").and_then(|v| v.strip_suffix(')')).is_some() {
    let mut rng = rand::thread_rng();
    if spec.is_float {
      (0..expected_len)
        .map(|_| Number::Float(rng.r#gen::<f32>()))
        .collect()
    } else {
      (0..expected_len)
        .map(|_| Number::Int(rng.gen_range(0..100) as i64))
        .collect()
    }
  } else {
    vec![parse_number(value)?]
  };

  if numbers.len() != expected_len {
    return Err(format!(
      "initializer produced {} values, expected {}",
      numbers.len(),
      expected_len
    ));
  }

  Ok(numbers)
}

fn parse_number_list(value: &str) -> Result<Vec<Number>, String> {
  let mut out = Vec::new();
  for token in value.split(',') {
    let token = token.trim();
    if token.is_empty() {
      continue;
    }
    out.push(parse_number(token)?);
  }
  Ok(out)
}

fn parse_call_tokens(value: &str) -> Vec<&str> {
  value
    .split(',')
    .map(|token| token.trim())
    .filter(|token| !token.is_empty())
    .collect()
}

pub(super) fn parse_file_initializer(value: &str) -> Result<Option<(String, TypeSpec)>, String> {
  let args = match value.strip_prefix("file(").and_then(|v| v.strip_suffix(')')) {
    Some(args) => args,
    None => return Ok(None),
  };
  let args = parse_call_tokens(args);
  if args.len() != 2 {
    return Err("file expects 2 args".to_string());
  }
  let path = parse_string_arg(args[0], "file path")?;
  let dtype = parse_type(args[1])?;
  Ok(Some((path, dtype)))
}

fn parse_string_arg(value: &str, label: &str) -> Result<String, String> {
  let value = value.trim();
  if value.len() < 2 {
    return Err(format!("{} must be a quoted string", label));
  }
  let bytes = value.as_bytes();
  let quote = bytes[0];
  if (quote != b'"' && quote != b'\'') || bytes[value.len() - 1] != quote {
    return Err(format!("{} must be a quoted string", label));
  }
  Ok(value[1..value.len() - 1].to_string())
}

fn parse_i32_arg(value: &str, label: &str) -> Result<i32, String> {
  match parse_number(value)? {
    Number::Int(v) => i32::try_from(v).map_err(|_| format!("{} out of range", label)),
    Number::Float(_) => Err(format!("{} must be an integer", label)),
  }
}

pub(super) fn parse_number(value: &str) -> Result<Number, String> {
  let value = value.trim();
  if value.contains('.') || value.contains('e') || value.contains('E') {
    let num = value
      .parse::<f32>()
      .map_err(|_| format!("invalid number '{}'", value))?;
    return Ok(Number::Float(num));
  }
  let (sign, rest) = if let Some(stripped) = value.strip_prefix('-') {
    (-1i64, stripped)
  } else {
    (1i64, value)
  };
  let parsed = if let Some(hex) = rest.strip_prefix("0x") {
    i64::from_str_radix(hex, 16).map_err(|_| format!("invalid hex '{}'", value))?
  } else if let Some(bin) = rest.strip_prefix("0b") {
    i64::from_str_radix(bin, 2).map_err(|_| format!("invalid bin '{}'", value))?
  } else {
    rest.parse::<i64>().map_err(|_| format!("invalid int '{}'", value))?
  };
  Ok(Number::Int(sign * parsed))
}

struct ValueEncoder<'a> {
  spec: &'a TypeSpec,
  bytes: Vec<u8>,
}

impl<'a> ValueEncoder<'a> {
  fn new(spec: &'a TypeSpec, capacity: usize) -> Self {
    Self {
      spec,
      bytes: Vec::with_capacity(capacity * spec.element_size()),
    }
  }

  fn encode_value(&mut self, value: &Number) -> Result<(), String> {
    if self.spec.is_float {
      let float_val = match *value {
        Number::Int(v) => v as f32,
        Number::Float(v) => v,
      };

      match (self.spec.bits, self.spec.is_bfloat) {
        (16, true) => {
          self.bytes.extend_from_slice(
            &bf16::from_f32(float_val).to_bits().to_le_bytes()
          );
        }
        (32, false) => {
          self.bytes.extend_from_slice(&float_val.to_le_bytes());
        }
        _ => {
          return Err(format!(
            "unsupported float type: {} bits, bfloat={}",
            self.spec.bits, self.spec.is_bfloat
          ));
        }
      }
    } else {
      let int_val = match *value {
        Number::Int(v) => v,
        Number::Float(v) => {
          if v.fract() != 0.0 {
            return Err(format!(
              "cannot convert {:.3} to integer type {}",
              v, self.spec.name
            ));
          }
          v as i64
        }
      };

      if self.spec.is_signed {
        match self.spec.bits {
          8 => self.bytes.extend_from_slice(&(int_val as i8).to_le_bytes()),
          16 => self.bytes.extend_from_slice(&(int_val as i16).to_le_bytes()),
          32 => self.bytes.extend_from_slice(&(int_val as i32).to_le_bytes()),
          64 => self.bytes.extend_from_slice(&int_val.to_le_bytes()),
          _ => {
            return Err(format!(
              "unsupported signed integer width: {} bits",
              self.spec.bits
            ));
          }
        }
      } else {
        if int_val < 0 {
          return Err("unsigned value must be >= 0".to_string());
        }
        let uint_val = int_val as u64;

        match self.spec.bits {
          8 => self.bytes.extend_from_slice(&(uint_val as u8).to_le_bytes()),
          16 => self.bytes.extend_from_slice(&(uint_val as u16).to_le_bytes()),
          32 => self.bytes.extend_from_slice(&(uint_val as u32).to_le_bytes()),
          64 => self.bytes.extend_from_slice(&uint_val.to_le_bytes()),
          _ => {
            return Err(format!(
              "unsupported unsigned integer width: {} bits",
              self.spec.bits
            ));
          }
        }
      }
    }

    Ok(())
  }

  fn into_bytes(self) -> Vec<u8> {
    self.bytes
  }
}

pub(super) fn encode_values(values: &[Number], spec: &TypeSpec) -> Result<Vec<u8>, String> {
  let mut encoder = ValueEncoder::new(spec, values.len());
  for value in values {
    encoder.encode_value(value)?;
  }
  Ok(encoder.into_bytes())
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::parse::test_support::{numbers_to_f32, numbers_to_i64, spec};

  fn assert_int(value: Number, expected: i64) {
    match value {
      Number::Int(v) => assert_eq!(v, expected),
      Number::Float(v) => panic!("expected int {}, got float {}", expected, v),
    }
  }

  #[test]
  fn parse_numbers() {
    assert_int(parse_number("0x2a").unwrap(), 42);
    assert_int(parse_number("0b1010").unwrap(), 10);
    assert_int(parse_number("-7").unwrap(), -7);
  }

  #[test]
  fn parse_arange() {
    let spec = spec("f32");
    let values = parse_initializer("arange(4)", &spec, 4, &[2, 2]).unwrap();
    assert_eq!(numbers_to_f32(&values), vec![0.0, 1.0, 2.0, 3.0]);
    let values = parse_initializer("arange(1, 13, 2)", &spec, 6, &[2, 3]).unwrap();
    assert_eq!(numbers_to_f32(&values), vec![1.0, 3.0, 5.0, 7.0, 9.0, 11.0]);
  }

  #[test]
  fn parse_initializer_errors() {
    let float_spec = spec("f32");
    let err = parse_initializer("arange(0, 2, 0)", &float_spec, 2, &[2]).unwrap_err();
    assert_eq!(err, "arange step cannot be 0");
    let unsigned_spec = spec("u32");
    let err = encode_values(&[Number::Int(-1)], &unsigned_spec).unwrap_err();
    assert_eq!(err, "unsigned value must be >= 0");
    let bf16_spec = spec("bf16");
    let values = parse_initializer("[1.0, -2.5]", &bf16_spec, 2, &[2]).unwrap();
    let bytes = encode_values(&values, &bf16_spec).unwrap();
    assert_eq!(bytes.len(), 4);
    assert_eq!(
      &bytes[0..2],
      &bf16::from_f32(1.0).to_bits().to_le_bytes()
    );
    assert_eq!(
      &bytes[2..4],
      &bf16::from_f32(-2.5).to_bits().to_le_bytes()
    );
  }

  #[test]
  fn parse_arange_variants() {
    let float_spec = spec("f32");
    let values = parse_initializer("arange(5)", &float_spec, 5, &[5]).unwrap();
    assert_eq!(numbers_to_f32(&values), vec![0.0, 1.0, 2.0, 3.0, 4.0]);

    let values = parse_initializer("arange(2, 6)", &float_spec, 4, &[4]).unwrap();
    assert_eq!(numbers_to_f32(&values), vec![2.0, 3.0, 4.0, 5.0]);

    let values = parse_initializer("arange(0, 6, 2)", &float_spec, 3, &[3]).unwrap();
    assert_eq!(numbers_to_f32(&values), vec![0.0, 2.0, 4.0]);

    let values = parse_initializer("arange(10, 6, -1)", &float_spec, 4, &[4]).unwrap();
    assert_eq!(numbers_to_f32(&values), vec![10.0, 9.0, 8.0, 7.0]);

    let err = parse_initializer("arange(0, 2.5, 1)", &float_spec, 5, &[5]).unwrap_err();
    assert_eq!(err, "arange end must be an integer");
  }

  #[test]
  fn parse_rand() {
    let float_spec = spec("f32");
    let values = parse_initializer("rand()", &float_spec, 5, &[5]).unwrap();
    assert_eq!(values.len(), 5);
    for val in &values {
      match val {
        Number::Float(f) => assert!(*f >= 0.0 && *f < 1.0),
        _ => panic!("expected float"),
      }
    }

    let int_spec = spec("i32");
    let values = parse_initializer("rand()", &int_spec, 10, &[10]).unwrap();
    assert_eq!(values.len(), 10);
    for val in &values {
      match val {
        Number::Int(i) => assert!(*i >= 0 && *i < 100),
        _ => panic!("expected int"),
      }
    }
  }

  #[test]
  fn parse_float_literals() {
    match parse_number("1.5").unwrap() {
      Number::Float(v) => assert!((v - 1.5).abs() < 1e-6),
      Number::Int(v) => panic!("expected float, got int {}", v),
    }

    match parse_number("1.5e2").unwrap() {
      Number::Float(v) => assert!((v - 150.0).abs() < 1e-6),
      Number::Int(v) => panic!("expected float, got int {}", v),
    }

    match parse_number("2.5E-1").unwrap() {
      Number::Float(v) => assert!((v - 0.25).abs() < 1e-6),
      Number::Int(v) => panic!("expected float, got int {}", v),
    }

    match parse_number("-3.14").unwrap() {
      Number::Float(v) => assert!((v + 3.14).abs() < 1e-6),
      Number::Int(v) => panic!("expected float, got int {}", v),
    }

    let float_spec = spec("f32");
    let values = parse_initializer("[1.1, 2.2, 3.3]", &float_spec, 3, &[3]).unwrap();
    let vals = numbers_to_f32(&values);
    assert!((vals[0] - 1.1).abs() < 1e-6);
    assert!((vals[1] - 2.2).abs() < 1e-6);
    assert!((vals[2] - 3.3).abs() < 1e-6);
  }

  #[test]
  fn parse_list_initializer() {
    let int_spec = spec("i32");
    let values = parse_initializer("[10, 20, 30, 40]", &int_spec, 4, &[4]).unwrap();
    assert_eq!(numbers_to_i64(&values), vec![10, 20, 30, 40]);

    let float_spec = spec("f32");
    let values = parse_initializer("[1.0, 2.0, 3.0]", &float_spec, 3, &[3]).unwrap();
    assert_eq!(numbers_to_f32(&values), vec![1.0, 2.0, 3.0]);

    let values = parse_initializer("[1, 2.5, 3, 4.5]", &float_spec, 4, &[4]).unwrap();
    assert_eq!(numbers_to_f32(&values), vec![1.0, 2.5, 3.0, 4.5]);

    let unsigned_spec = spec("u32");
    let values = parse_initializer("[0x10, 0x20, 0x30]", &unsigned_spec, 3, &[3]).unwrap();
    assert_eq!(numbers_to_i64(&values), vec![16, 32, 48]);
  }

  #[test]
  fn parse_repeat_initializer() {
    let values = parse_initializer("repeat(3)", &spec("i32"), 4, &[4]).unwrap();
    assert_eq!(numbers_to_i64(&values), vec![3, 3, 3, 3]);
    let values = parse_initializer("repeat(1.5)", &spec("f32"), 2, &[2]).unwrap();
    assert_eq!(numbers_to_f32(&values), vec![1.5, 1.5]);
    let err = parse_initializer("repeat(1, 2)", &spec("i32"), 2, &[2]).unwrap_err();
    assert_eq!(err, "repeat expects 1 arg");
  }

  #[test]
  fn parse_integer_widths() {
    let i8_values = parse_initializer("[-128, -1, 0, 127]", &spec("i8"), 4, &[4]).unwrap();
    assert_eq!(numbers_to_i64(&i8_values), vec![-128, -1, 0, 127]);

    let u8_values = parse_initializer("[0, 1, 128, 255]", &spec("u8"), 4, &[4]).unwrap();
    assert_eq!(numbers_to_i64(&u8_values), vec![0, 1, 128, 255]);

    let i16_values = parse_initializer("[-32768, 0, 32767]", &spec("i16"), 3, &[3]).unwrap();
    assert_eq!(numbers_to_i64(&i16_values), vec![-32768, 0, 32767]);

    let u16_values = parse_initializer("[0, 1000, 65535]", &spec("u16"), 3, &[3]).unwrap();
    assert_eq!(numbers_to_i64(&u16_values), vec![0, 1000, 65535]);

    let i64_values = parse_initializer(
      "[-9223372036854775807, 0, 9223372036854775807]",
      &spec("i64"),
      3,
      &[3],
    )
    .unwrap();
    assert_eq!(numbers_to_i64(&i64_values), vec![i64::MIN + 1, 0, i64::MAX]);

    let u64_values = parse_initializer(
      "[0, 9223372036854775807, 0x7FFFFFFFFFFFFFFF]",
      &spec("u64"),
      3,
      &[3],
    )
    .unwrap();
    assert_eq!(numbers_to_i64(&u64_values), vec![0, i64::MAX, i64::MAX]);
  }
}
