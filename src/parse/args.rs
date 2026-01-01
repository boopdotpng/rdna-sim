use std::{fmt, fs};

use crate::sim::{DecodedInst, MemoryOps};
use crate::{Dim3, Program, WaveSize};

use super::init::{encode_values, parse_file_initializer, parse_initializer};

#[derive(Clone, Debug)]
pub struct ArgInfo {
  pub name: String,
  pub type_name: String,
  pub shape: Vec<usize>,
  pub addr: u64,
  pub len: usize,
}

impl fmt::Display for ArgInfo {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(
      f,
      "{} : {} @ 0x{:x} in global mem",
      self.name, self.type_name, self.addr
    )
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

#[derive(Clone, Debug)]
pub(super) struct TypeSpec {
  pub name: String,
  pub bits: usize,
  pub is_float: bool,
  pub is_signed: bool,
  pub is_bfloat: bool,
}

impl TypeSpec {
  pub fn element_size(&self) -> usize {
    self.bits / 8
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
  let (spec, shape, type_name) = parse_type_and_shape(type_part)?;
  let len = shape.iter().product::<usize>().max(1);
  let byte_len = len
    .checked_mul(spec.element_size())
    .ok_or_else(|| "argument size overflow".to_string())?;

  let addr = program.global_mem.alloc(byte_len, spec.element_size())?;
  if let Some(init) = init_part {
    if let Some((path, file_spec)) = parse_file_initializer(init)? {
      if !same_type(&spec, &file_spec) {
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
      let values = parse_initializer(init, &spec, len, &shape)?;
      let bytes = encode_values(&values, &spec)?;
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
    type_name,
    shape,
    addr,
    len,
  })
}

fn parse_type_and_shape(value: &str) -> Result<(TypeSpec, Vec<usize>, String), String> {
  let trimmed = value.trim();
  if let Some(start) = trimmed.find('[') {
    let end = trimmed.find(']').ok_or_else(|| "missing ']' in type".to_string())?;
    let base = trimmed[..start].trim();
    let shape_str = trimmed[start + 1..end].trim();
    let shape = parse_shape(shape_str)?;
    let spec = parse_type(base)?;
    let type_name = format!("{}[{}]", base, shape_str);
    Ok((spec, shape, type_name))
  } else {
    let spec = parse_type(trimmed)?;
    Ok((spec, Vec::new(), trimmed.to_string()))
  }
}

fn parse_shape(value: &str) -> Result<Vec<usize>, String> {
  let mut out = Vec::new();
  for token in value.split(',') {
    let token = token.trim();
    if token.is_empty() {
      continue;
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

pub(super) fn parse_type(value: &str) -> Result<TypeSpec, String> {
  let value = value.trim();
  let lower = value.to_ascii_lowercase();
  if lower == "bf16" {
    return Ok(TypeSpec {
      name: lower,
      bits: 16,
      is_float: true,
      is_signed: true,
      is_bfloat: true,
    });
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
  Ok(TypeSpec {
    name: lower,
    bits,
    is_float,
    is_signed,
    is_bfloat: false,
  })
}

fn same_type(a: &TypeSpec, b: &TypeSpec) -> bool {
  a.name == b.name
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
    let s1 = parse_argument("s1", "i32", &mut program).unwrap();
    assert_eq!(s1.len, 1);
    assert_eq!(s1.type_name, "i32");

    let s2 = parse_argument("s2", "f32", &mut program).unwrap();
    assert_eq!(s2.len, 1);
    assert_eq!(s2.type_name, "f32");

    let s3 = parse_argument("s3", "u64", &mut program).unwrap();
    assert_eq!(s3.len, 1);
    assert_eq!(s3.type_name, "u64");
  }

  #[test]
  fn parse_uninitialized_array() {
    let mut program = program();
    let a1 = parse_argument("a1", "i32[4]", &mut program).unwrap();
    assert_eq!(a1.shape, vec![4]);
    assert_eq!(a1.len, 4);

    let a2 = parse_argument("a2", "f32[2,3]", &mut program).unwrap();
    assert_eq!(a2.shape, vec![2, 3]);
    assert_eq!(a2.len, 6);

    let a3 = parse_argument("a3", "u8[8]", &mut program).unwrap();
    assert_eq!(a3.shape, vec![8]);
    assert_eq!(a3.len, 8);
  }

  #[test]
  fn parse_multidimensional_shapes() {
    let mut program = program();
    let a1 = parse_argument("a1", "i32[2,3,4] = arange(24)", &mut program).unwrap();
    assert_eq!(a1.shape, vec![2, 3, 4]);
    assert_eq!(a1.len, 24);

    let a2 = parse_argument("a2", "f32[2,2,2,2] = arange(16)", &mut program).unwrap();
    assert_eq!(a2.shape, vec![2, 2, 2, 2]);
    assert_eq!(a2.len, 16);
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
