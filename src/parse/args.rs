use std::{fmt, fs};

use crate::sim::{DecodedInst, MemoryOps};
use crate::{Dim3, Program, WaveSize};

use super::init::{encode_values, parse_file_initializer, parse_initializer};

#[derive(Clone, Debug)]
pub struct ArgInfo {
  pub name: String,
  pub arg_type: ArgType,
  pub addr: u64,
  pub len: usize,
  pub shape: Vec<usize>,
}

impl fmt::Display for ArgInfo {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(
      f,
      "{} : {}",
      self.name, self.arg_type
    )?;
    if !self.shape.is_empty() {
      write!(f, "[")?;
      for (idx, dim) in self.shape.iter().enumerate() {
        if idx > 0 {
          write!(f, ",")?;
        }
        write!(f, "{}", dim)?;
      }
      write!(f, "]")?;
    }
    write!(f, " @ 0x{:x} in global mem", self.addr)
  }
}

#[derive(Clone, Debug)]
pub struct ProgramInfo {
  pub instructions: Vec<DecodedInst>,
  pub arguments: Vec<ArgInfo>,
  pub output_arguments: Vec<ArgInfo>,
  pub local_launch_size: Dim3,
  pub global_launch_size: Dim3,
  pub wave_size: Option<WaveSize>,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ArgType {
  U8,
  I8,
  U16,
  I16,
  U32,
  I32,
  U64,
  I64,
  F32,
  BF16,
}

impl ArgType {
  pub fn element_size(self) -> usize {
    self.bits() / 8
  }

  pub fn bits(self) -> usize {
    match self {
      ArgType::U8 | ArgType::I8 => 8,
      ArgType::U16 | ArgType::I16 | ArgType::BF16 => 16,
      ArgType::U32 | ArgType::I32 | ArgType::F32 => 32,
      ArgType::U64 | ArgType::I64 => 64,
    }
  }

  pub fn is_float(self) -> bool {
    matches!(self, ArgType::F32 | ArgType::BF16)
  }

  pub fn is_signed(self) -> bool {
    matches!(self, ArgType::I8 | ArgType::I16 | ArgType::I32 | ArgType::I64 | ArgType::F32 | ArgType::BF16)
  }

  pub fn is_bfloat(self) -> bool {
    matches!(self, ArgType::BF16)
  }

  pub fn name(self) -> &'static str {
    match self {
      ArgType::U8 => "u8",
      ArgType::I8 => "i8",
      ArgType::U16 => "u16",
      ArgType::I16 => "i16",
      ArgType::U32 => "u32",
      ArgType::I32 => "i32",
      ArgType::U64 => "u64",
      ArgType::I64 => "i64",
      ArgType::F32 => "f32",
      ArgType::BF16 => "bf16",
    }
  }
}

impl fmt::Display for ArgType {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str(self.name())
  }
}

pub(super) fn parse_argument(
  name: &str,
  value: &str,
  program: &mut Program,
) -> Result<ArgInfo, String> {
  let (type_part, init_part) = if let Some((left, right)) = value.split_once('=') {
    (left.trim(), Some(right.trim()))
  } else {
    (value.trim(), None)
  };
  let (arg_type, shape) = parse_type_and_shape(type_part)?;
  let len = shape.iter().product::<usize>().max(1);
  let byte_len = len
    .checked_mul(arg_type.element_size())
    .ok_or_else(|| "argument size overflow".to_string())?;

  let addr = program.global_mem.alloc(byte_len, arg_type.element_size())?;
  if let Some(init) = init_part {
    if let Some((path, file_spec)) = parse_file_initializer(init)? {
      if arg_type != file_spec {
        return Err("file dtype must match declared type".to_string());
      }
      let bytes = fs::read(&path)
        .map_err(|e| format!("failed to read file {}: {}", path, e))?;
      if bytes.len() != byte_len {
        return Err(format!(
          "file initializer produced {} bytes, expected {}",
          bytes.len(),
          byte_len
        ));
      }
      program.global_mem.write(addr, &bytes)?;
    } else {
      let values = parse_initializer(init, &arg_type, len, &shape)?;
      let bytes = encode_values(&values, &arg_type)?;
      if bytes.len() != byte_len {
        return Err(format!(
          "initializer for '{}' produced {} bytes, expected {}",
          name,
          bytes.len(),
          byte_len
        ));
      }
      program.global_mem.write(addr, &bytes)?;
    }
  } else {
    program.global_mem.write_zeros(addr, byte_len)?;
  }

  Ok(ArgInfo {
    name: name.to_string(),
    arg_type,
    addr,
    len,
    shape,
  })
}

fn parse_type_and_shape(value: &str) -> Result<(ArgType, Vec<usize>), String> {
  let trimmed = value.trim();
  if let Some(start) = trimmed.find('[') {
    let end = trimmed.find(']').ok_or_else(|| "missing ']' in type".to_string())?;
    let base = trimmed[..start].trim();
    let shape_str = trimmed[start + 1..end].trim();
    let shape = parse_shape(shape_str)?;
    let spec = parse_type(base)?;
    Ok((spec, shape))
  } else {
    let spec = parse_type(trimmed)?;
    Ok((spec, Vec::new()))
  }
}

fn parse_shape(value: &str) -> Result<Vec<usize>, String> {
  let mut out = Vec::new();
  for token in value.split(',') {
    let token = token.trim();
    if token.is_empty() {
      return Err("empty shape dim".to_string());
    }
    let dim = token
      .parse::<usize>()
      .map_err(|_| format!("invalid shape dim '{}'", token))?;
    if dim == 0 {
      return Err("shape dims must be >= 1".to_string());
    }
    out.push(dim);
  }
  if out.is_empty() {
    return Err("empty shape".to_string());
  }
  Ok(out)
}

pub(super) fn parse_type(value: &str) -> Result<ArgType, String> {
  let value = value.trim();
  let lower = value.to_ascii_lowercase();
  if lower == "bf16" {
    return Ok(ArgType::BF16);
  }
  if lower.len() < 2 {
    return Err(format!("invalid type '{}'", value));
  }
  let (prefix, digits) = lower.split_at(1);
  let bits = digits
    .parse::<usize>()
    .map_err(|_| format!("invalid type '{}'", value))?;
  let (is_float, is_signed) = match prefix {
    "f" => (true, true),
    "i" => (false, true),
    "u" => (false, false),
    _ => return Err(format!("unsupported type '{}'", value)),
  };
  if bits % 8 != 0 {
    return Err(format!("unsupported type width '{}'", value));
  }
  if is_float && bits != 32 {
    return Err(format!("unsupported float type '{}'", value));
  }
  match (is_float, is_signed, bits) {
    (true, _, 32) => Ok(ArgType::F32),
    (false, true, 8) => Ok(ArgType::I8),
    (false, true, 16) => Ok(ArgType::I16),
    (false, true, 32) => Ok(ArgType::I32),
    (false, true, 64) => Ok(ArgType::I64),
    (false, false, 8) => Ok(ArgType::U8),
    (false, false, 16) => Ok(ArgType::U16),
    (false, false, 32) => Ok(ArgType::U32),
    (false, false, 64) => Ok(ArgType::U64),
    _ => Err(format!("unsupported type '{}'", value)),
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::parse::test_support::{program, spec};
  use crate::parse::init::{encode_values, Number};
  use std::fs;

  #[test]
  fn parse_shapes() {
    let err = parse_type_and_shape("i32[0]").unwrap_err();
    assert_eq!(err, "shape dims must be >= 1");

    let err = parse_type_and_shape("i32[]").unwrap_err();
    assert_eq!(err, "empty shape dim");

    let err = parse_type_and_shape("i32[1,a]").unwrap_err();
    assert_eq!(err, "invalid shape dim 'a'");

    let err = parse_type_and_shape("f32[16, , 3]").unwrap_err();
    assert_eq!(err, "empty shape dim");
  }

  #[test]
  fn parse_file_initializer() {
    let mut program = program();
    let path = crate::parse::test_support::temp_path("bin");
    fs::write(&path, vec![1u8, 0, 0, 0]).expect("write bin");
    let init = format!("u32 = file(\"{}\", u32)", path.display());
    let arg = parse_argument("arg", &init, &mut program).unwrap();
    assert_eq!(arg.len, 1);
    let init = format!("u32 = file(\"{}\", i32)", path.display());
    let err = parse_argument("arg", &init, &mut program).unwrap_err();
    assert_eq!(err, "file dtype must match declared type");
    fs::write(&path, vec![1u8, 2]).expect("write bin");
    let init = format!("u32 = file(\"{}\", u32)", path.display());
    let err = parse_argument("arg", &init, &mut program).unwrap_err();
    assert_eq!(err, "file initializer produced 2 bytes, expected 4");
  }

  #[test]
  fn parse_uninitialized_scalar() {
    let mut program = program();
    for (name, arg_type, spec) in [
      ("s1", ArgType::I32, "i32"),
      ("s2", ArgType::F32, "f32"),
      ("s3", ArgType::U64, "u64"),
    ] {
      let arg = parse_argument(name, spec, &mut program).unwrap();
      assert_eq!(arg.len, 1);
      assert_eq!(arg.arg_type, arg_type);
      assert!(arg.shape.is_empty());
    }
  }

  #[test]
  fn parse_array_lengths() {
    let mut program = program();
    for (name, spec, len) in [
      ("a1", "i32[4]", 4),
      ("a2", "f32[2,3]", 6),
      ("a3", "u8[8]", 8),
      ("a4", "i32[2,3,4] = arange(24)", 24),
      ("a5", "f32[2,2,2,2] = arange(16)", 16),
    ] {
      let arg = parse_argument(name, spec, &mut program).unwrap();
      assert_eq!(arg.len, len);
    }
  }

  #[test]
  fn test_value_range_validation() {
    let mut prog1 = program();

    let arg = parse_argument("a", "u8 = 255", &mut prog1).unwrap();
    assert_eq!(arg.len, 1);

    let bytes = encode_values(&[Number::Int(256)], &spec("u8"));
    if let Ok(bytes) = bytes {
      assert_eq!(bytes[0], 0);
    }

    let mut prog2 = program();
    let arg = parse_argument("b", "i8 = -128", &mut prog2).unwrap();
    assert_eq!(arg.len, 1);

    let mut prog3 = program();
    let arg = parse_argument("c", "i8 = 127", &mut prog3).unwrap();
    assert_eq!(arg.len, 1);

    let bytes = encode_values(&[Number::Int(-129)], &spec("i8"));
    if let Ok(bytes) = bytes {
      assert_eq!(bytes[0] as i8, 127);
    }

    let mut prog4 = program();
    let arg = parse_argument("d", "u16 = 65535", &mut prog4).unwrap();
    assert_eq!(arg.len, 1);

    let mut prog5 = program();
    let arg = parse_argument("e", "u32 = 4294967295", &mut prog5).unwrap();
    assert_eq!(arg.len, 1);

    let mut prog6 = program();
    let err = parse_argument("f", "u8 = -1", &mut prog6).unwrap_err();
    assert_eq!(err, "unsigned value must be >= 0");

    let mut prog7 = program();
    let err = parse_argument("g", "u32 = -100", &mut prog7).unwrap_err();
    assert_eq!(err, "unsigned value must be >= 0");
  }

}
