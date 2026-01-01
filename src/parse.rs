use std::fs;
use std::path::Path;
use std::str::FromStr;

use half::bf16;

use crate::decode::{decode_instruction, format_decode_error};
use crate::isa::types::InstructionCommonDef;
use crate::parse_instruction::parse_instruction;
use crate::sim::{generate_arange, DecodedInst};
use crate::{Architecture, Dim3, Program, WaveSize};

#[derive(Clone, Debug)]
pub struct ArgInfo {
    pub name: String,
    pub type_name: String,
    pub shape: Vec<usize>, // user can pass in [3,3] as a shape, we need to save that per argument so that it prints neatly at the end
    pub addr: u64, 
    pub len: usize,
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
struct TypeSpec {
  name: String,
  bits: usize,
  is_float: bool,
  is_signed: bool,
  is_bfloat: bool,
}

impl TypeSpec {
  fn element_size(&self) -> usize { self.bits / 8 }
}

#[derive(Clone, Debug)]
enum Number {
    Int(i64),
    Float(f32),
}

pub fn parse_file(
    file_path: &Path,
    program: &mut Program,
    arch: Architecture,
) -> Result<ProgramInfo, String> {
    let content = fs::read_to_string(file_path)
        .map_err(|e| format!("Failed to read file {}: {}", file_path.display(), e))?;

    let mut instructions = Vec::new();
    let mut arguments = Vec::new();
    let mut output_arguments = Vec::new();
    let mut local_launch_size = Dim3::new(64, 1, 1);
    let mut global_launch_size = Dim3::new(64, 1, 1);
    let mut wave_size = None;

    let mut in_header = false;
    for (line_no, raw) in content.lines().enumerate() {
        let line = strip_comments(raw).trim();
        if line.is_empty() {
            continue;
        }
        if line == "---" {
            in_header = !in_header;
            continue;
        }
        if in_header {
            if let Some((key, value)) = split_key_value(line) {
                let key = key.trim();
                let value = value.trim();
                match key {
                    "local" => {
                        local_launch_size = Dim3::from_str(value).map_err(|e| {
                            format!("line {}: invalid local size: {}", line_no + 1, e)
                        })?;
                    }
                    "global" => {
                        global_launch_size = Dim3::from_str(value).map_err(|e| {
                            format!("line {}: invalid global size: {}", line_no + 1, e)
                        })?;
                    }
                    "wave" => {
                        let val = value.parse::<u32>().map_err(|_| {
                            format!("line {}: invalid wave size '{}'", line_no + 1, value)
                        })?;
                        // update to include wave64 support when added
                        wave_size = match val {
                            32 => Some(WaveSize::Wave32),
                            64 => {
                                return Err(format!(
                                    "line {}: wave size 64 not supported yet",
                                    line_no + 1
                                ));
                            }
                            _ => {
                                return Err(format!(
                                    "line {}: wave size must be 32",
                                    line_no + 1
                                ));
                            }
                        };
                    }
                    _ => {
                        let arg = parse_argument(key, value, program).map_err(|e| {
                            format!("line {}: {}", line_no + 1, e)
                        })?;
                        if arg.name.starts_with("out_") {
                            output_arguments.push(arg);
                        } else {
                            arguments.push(arg);
                        }
                    }
                }
            } else {
                return Err(format!("line {}: malformed header entry", line_no + 1));
            }
        } else {
            // Skip print directives for now (will be handled later via wavestate)
            if line.starts_with("print") {
                continue;
            }

            // Parse the instruction text
            let parsed = parse_instruction(line)
                .map_err(|e| format!("line {}: {}", line_no + 1, e))?;

            // Look up instruction definition
            let def = lookup_instruction_def(&parsed.name, arch)
                .ok_or_else(|| format!("line {}: unknown instruction '{}'",
                                       line_no + 1, parsed.name))?;

            // Decode and validate
            let decoded = decode_instruction(&parsed, def, line_no + 1)
                .map_err(|e| format_decode_error(e))?;

            instructions.push(decoded);
        }
    }

    Ok(ProgramInfo {
        instructions,
        arguments,
        output_arguments,
        local_launch_size,
        global_launch_size,
        wave_size,
    })
}

fn strip_comments(line: &str) -> &str {
    let mut cut = line.len();
    // traditional comments in assembly, # is non-standard
    if let Some(idx) = line.find("//") {
        cut = cut.min(idx);
    }
    if let Some(idx) = line.find(';') {
        cut = cut.min(idx);
    }
    &line[..cut]
}

fn split_key_value(line: &str) -> Option<(&str, &str)> {
    let mut best: Option<(usize, char)> = None;
    for (idx, ch) in line.char_indices() {
        if ch == ':' || ch == '=' {
            best = Some((idx, ch));
            break;
        }
    }
    best.map(|(idx, _)| (&line[..idx], &line[idx + 1..]))
}

fn parse_argument(name: &str, value: &str, program: &mut Program) -> Result<ArgInfo, String> {
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

  let addr = program.alloc_global(byte_len, spec.element_size())?;
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
      program.write_global(addr, &bytes)?;
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
      program.write_global(addr, &bytes)?;
    }
  } else {
    program.write_global_zeros(addr, byte_len)?;
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

fn parse_type(value: &str) -> Result<TypeSpec, String> {
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

fn parse_initializer(
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

fn parse_file_initializer(value: &str) -> Result<Option<(String, TypeSpec)>, String> {
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

fn same_type(a: &TypeSpec, b: &TypeSpec) -> bool {
  a.name == b.name
}

fn parse_i32_arg(value: &str, label: &str) -> Result<i32, String> {
    match parse_number(value)? {
        Number::Int(v) => i32::try_from(v).map_err(|_| format!("{} out of range", label)),
        Number::Float(_) => Err(format!("{} must be an integer", label)),
    }
}

fn parse_number(value: &str) -> Result<Number, String> {
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

fn encode_values(values: &[Number], spec: &TypeSpec) -> Result<Vec<u8>, String> {
    let mut encoder = ValueEncoder::new(spec, values.len());
    for value in values {
        encoder.encode_value(value)?;
    }
    Ok(encoder.into_bytes())
}

fn lookup_instruction_def(name: &str, arch: Architecture) -> Option<&'static InstructionCommonDef> {
    // First check base ISA (instructions common to all architectures)
    if let Some(common_def) = crate::isa::base::lookup_common_normalized(name) {
        return Some(common_def);
    }

    // Then check arch-specific instructions
    match arch {
        Architecture::Rdna35 => {
            crate::isa::rdna35::lookup_common_def(name)
        }
    }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::{Dim3, Program, WaveSize};
  use std::fs;
  use std::path::PathBuf;
  use std::sync::atomic::{AtomicUsize, Ordering};

  static TEMP_COUNTER: AtomicUsize = AtomicUsize::new(0);

  fn temp_path(name: &str) -> PathBuf {
    let id = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let mut path = std::env::temp_dir();
    path.push(format!(
      "rdna_parse_test_{}_{}_{}.rdna",
      std::process::id(),
      name,
      id
    ));
    path
  }

  fn write_temp(contents: &str, name: &str) -> PathBuf {
    let path = temp_path(name);
    fs::write(&path, contents).expect("write temp rdna");
    path
  }

  fn program() -> Program {
    Program::new(1024, Dim3::new(1, 1, 1), Dim3::new(1, 1, 1), WaveSize::Wave32)
  }

  fn spec(name: &str) -> TypeSpec {
    parse_type(name).expect("type spec")
  }

  fn numbers_to_f32(values: &[Number]) -> Vec<f32> {
    values
      .iter()
      .map(|value| match *value {
        Number::Int(v) => v as f32,
        Number::Float(v) => v,
      })
      .collect()
  }

  fn numbers_to_i64(values: &[Number]) -> Vec<i64> {
    values
      .iter()
      .map(|value| match *value {
        Number::Int(v) => v,
        Number::Float(v) => v as i64,
      })
      .collect()
  }

  fn assert_int(value: Number, expected: i64) {
    match value {
      Number::Int(v) => assert_eq!(v, expected),
      Number::Float(v) => panic!("expected int {}, got float {}", expected, v),
    }
  }

  #[test]
  fn parse_file_header_and_instructions() {
    let contents = r#"
    ---
    arg_a: i32 = 3
    out_y: f32[2,2]
    local = 2, 1, 1
    global = 3, 1, 1
    wave = 32
    ---
    s_mov_b32 s0, 0 // comment
    s_waitcnt lgkmcnt(0) vmcnt(0)
    ; another comment
    "#;
    let path = write_temp(contents, "header");
    let mut program = program();
    let info = parse_file(&path, &mut program, Architecture::Rdna35).expect("parse file");
    assert_eq!(info.local_launch_size, Dim3::new(2, 1, 1));
    assert_eq!(info.global_launch_size, Dim3::new(3, 1, 1));
    assert_eq!(info.wave_size, Some(WaveSize::Wave32));
    assert_eq!(info.arguments.len(), 1);
    assert_eq!(info.output_arguments.len(), 1);
    assert_eq!(info.arguments[0].name, "arg_a");
    assert_eq!(info.output_arguments[0].name, "out_y");
    assert_eq!(info.instructions.len(), 2);
    assert_eq!(info.instructions[0].name, "s_mov_b32");
    assert_eq!(info.instructions[1].name, "s_waitcnt");
  }

  #[test]
  fn parse_numbers_and_shapes() {
    assert_int(parse_number("0x2a").unwrap(), 42);
    assert_int(parse_number("0b1010").unwrap(), 10);
    assert_int(parse_number("-7").unwrap(), -7);
    let err = parse_type_and_shape("i32[0]").unwrap_err();
    assert_eq!(err, "shape dims must be >= 1");
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
  fn parse_wave_size_validation() {
    let contents = r#"
    ---
    wave = 64
    ---
    s_endpgm
    "#;
    let path = write_temp(contents, "wave64");
    let mut program = program();
    let err = parse_file(&path, &mut program, Architecture::Rdna35).unwrap_err();
    assert_eq!(err, "line 3: wave size 64 not supported yet");
  }

  #[test]
  fn parse_arange_variants() {
    let float_spec = spec("f32");
    // arange(end) - single arg, already tested elsewhere but included for completeness
    let values = parse_initializer("arange(5)", &float_spec, 5, &[5]).unwrap();
    assert_eq!(numbers_to_f32(&values), vec![0.0, 1.0, 2.0, 3.0, 4.0]);

    // arange(start, end) - 2-arg form
    let values = parse_initializer("arange(2, 6)", &float_spec, 4, &[4]).unwrap();
    assert_eq!(numbers_to_f32(&values), vec![2.0, 3.0, 4.0, 5.0]);

    // arange(start, end, step) - 3-arg form
    let values = parse_initializer("arange(0, 6, 2)", &float_spec, 3, &[3]).unwrap();
    assert_eq!(numbers_to_f32(&values), vec![0.0, 2.0, 4.0]);

    // arange with negative step
    let values = parse_initializer("arange(10, 6, -1)", &float_spec, 4, &[4]).unwrap();
    assert_eq!(numbers_to_f32(&values), vec![10.0, 9.0, 8.0, 7.0]);

    // arange with float args should error
    let err = parse_initializer("arange(0, 2.5, 1)", &float_spec, 5, &[5]).unwrap_err();
    assert_eq!(err, "arange end must be an integer");
  }

  #[test]
  fn parse_rand() {
    let float_spec = spec("f32");
    // rand() generates random floats in [0.0, 1.0)
    let values = parse_initializer("rand()", &float_spec, 5, &[5]).unwrap();
    assert_eq!(values.len(), 5);
    // Check all are floats in valid range
    for val in &values {
      match val {
        Number::Float(f) => assert!(*f >= 0.0 && *f < 1.0),
        _ => panic!("expected float"),
      }
    }

    let int_spec = spec("i32");
    // rand() generates random integers in [0, 100)
    let values = parse_initializer("rand()", &int_spec, 10, &[10]).unwrap();
    assert_eq!(values.len(), 10);
    // Check all are ints in valid range
    for val in &values {
      match val {
        Number::Int(i) => assert!(*i >= 0 && *i < 100),
        _ => panic!("expected int"),
      }
    }
  }

  #[test]
  fn parse_float_literals() {
    // simple float
    match parse_number("1.5").unwrap() {
      Number::Float(v) => assert!((v - 1.5).abs() < 1e-6),
      Number::Int(v) => panic!("expected float, got int {}", v),
    }

    // scientific notation lowercase
    match parse_number("1.5e2").unwrap() {
      Number::Float(v) => assert!((v - 150.0).abs() < 1e-6),
      Number::Int(v) => panic!("expected float, got int {}", v),
    }

    // scientific notation uppercase
    match parse_number("2.5E-1").unwrap() {
      Number::Float(v) => assert!((v - 0.25).abs() < 1e-6),
      Number::Int(v) => panic!("expected float, got int {}", v),
    }

    // negative float
    match parse_number("-3.14").unwrap() {
      Number::Float(v) => assert!((v + 3.14).abs() < 1e-6),
      Number::Int(v) => panic!("expected float, got int {}", v),
    }

    // float in array
    let float_spec = spec("f32");
    let values = parse_initializer("[1.1, 2.2, 3.3]", &float_spec, 3, &[3]).unwrap();
    let vals = numbers_to_f32(&values);
    assert!((vals[0] - 1.1).abs() < 1e-6);
    assert!((vals[1] - 2.2).abs() < 1e-6);
    assert!((vals[2] - 3.3).abs() < 1e-6);
  }

  #[test]
  fn parse_list_initializer() {
    // Integer list
    let int_spec = spec("i32");
    let values = parse_initializer("[10, 20, 30, 40]", &int_spec, 4, &[4]).unwrap();
    assert_eq!(numbers_to_i64(&values), vec![10, 20, 30, 40]);

    // Float list
    let float_spec = spec("f32");
    let values = parse_initializer("[1.0, 2.0, 3.0]", &float_spec, 3, &[3]).unwrap();
    assert_eq!(numbers_to_f32(&values), vec![1.0, 2.0, 3.0]);

    // Mixed int/float in float array (ints coerced to float)
    let values = parse_initializer("[1, 2.5, 3, 4.5]", &float_spec, 4, &[4]).unwrap();
    assert_eq!(numbers_to_f32(&values), vec![1.0, 2.5, 3.0, 4.5]);

    // Hex values in list
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
  fn parse_file_initializer() {
    let mut program = program();
    let path = temp_path("bin");
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
    // Uninitialized i32 scalar should parse
    let s1 = parse_argument("s1", "i32", &mut program).unwrap();
    assert_eq!(s1.len, 1);
    assert_eq!(s1.type_name, "i32");

    // Uninitialized f32 scalar should parse
    let s2 = parse_argument("s2", "f32", &mut program).unwrap();
    assert_eq!(s2.len, 1);
    assert_eq!(s2.type_name, "f32");

    // Uninitialized u64 scalar should parse
    let s3 = parse_argument("s3", "u64", &mut program).unwrap();
    assert_eq!(s3.len, 1);
    assert_eq!(s3.type_name, "u64");
  }

  #[test]
  fn parse_uninitialized_array() {
    let mut program = program();
    // Uninitialized i32 array should parse
    let a1 = parse_argument("a1", "i32[4]", &mut program).unwrap();
    assert_eq!(a1.shape, vec![4]);
    assert_eq!(a1.len, 4);

    // Uninitialized f32 array should parse
    let a2 = parse_argument("a2", "f32[2,3]", &mut program).unwrap();
    assert_eq!(a2.shape, vec![2, 3]);
    assert_eq!(a2.len, 6);

    // Uninitialized u8 array should parse
    let a3 = parse_argument("a3", "u8[8]", &mut program).unwrap();
    assert_eq!(a3.shape, vec![8]);
    assert_eq!(a3.len, 8);
  }

  #[test]
  fn parse_output_array_zero_initialized() {
    let contents = r#"
    ---
    out_result: f32[4]
    out_matrix: i32[2,2]
    ---
    s_endpgm
    "#;
    let path = write_temp(contents, "output_zeros");
    let mut program = program();
    let info = parse_file(&path, &mut program, Architecture::Rdna35).expect("parse file");

    assert_eq!(info.output_arguments.len(), 2);

    let out1 = &info.output_arguments[0];
    assert_eq!(out1.name, "out_result");
    assert_eq!(out1.len, 4);

    let out2 = &info.output_arguments[1];
    assert_eq!(out2.name, "out_matrix");
    assert_eq!(out2.len, 4);
  }

  #[test]
  fn parse_multidimensional_shapes() {
    let mut program = program();
    // 3D shape
    let a1 = parse_argument("a1", "i32[2,3,4] = arange(24)", &mut program).unwrap();
    assert_eq!(a1.shape, vec![2, 3, 4]);
    assert_eq!(a1.len, 24);

    // 4D shape
    let a2 = parse_argument("a2", "f32[2,2,2,2] = arange(16)", &mut program).unwrap();
    assert_eq!(a2.shape, vec![2, 2, 2, 2]);
    assert_eq!(a2.len, 16);
  }

  #[test]
  fn parse_integer_widths() {
    // i8
    let i8_values = parse_initializer("[-128, -1, 0, 127]", &spec("i8"), 4, &[4]).unwrap();
    assert_eq!(numbers_to_i64(&i8_values), vec![-128, -1, 0, 127]);

    // u8
    let u8_values = parse_initializer("[0, 1, 128, 255]", &spec("u8"), 4, &[4]).unwrap();
    assert_eq!(numbers_to_i64(&u8_values), vec![0, 1, 128, 255]);

    // i16
    let i16_values = parse_initializer("[-32768, 0, 32767]", &spec("i16"), 3, &[3]).unwrap();
    assert_eq!(numbers_to_i64(&i16_values), vec![-32768, 0, 32767]);

    // u16
    let u16_values = parse_initializer("[0, 1000, 65535]", &spec("u16"), 3, &[3]).unwrap();
    assert_eq!(numbers_to_i64(&u16_values), vec![0, 1000, 65535]);

    // i64 (note: i64::MIN cannot be parsed due to sign stripping, so use slightly smaller value)
    let i64_values = parse_initializer(
      "[-9223372036854775807, 0, 9223372036854775807]",
      &spec("i64"),
      3,
      &[3],
    )
    .unwrap();
    assert_eq!(numbers_to_i64(&i64_values), vec![i64::MIN + 1, 0, i64::MAX]);

    // u64 (note: parser uses i64 internally, so max representable is i64::MAX)
    let u64_values = parse_initializer(
      "[0, 9223372036854775807, 0x7FFFFFFFFFFFFFFF]",
      &spec("u64"),
      3,
      &[3],
    )
    .unwrap();
    assert_eq!(numbers_to_i64(&u64_values), vec![0, i64::MAX, i64::MAX]);
  }

  #[test]
  fn parse_full_program_with_memory_verification() {
    let contents = r#"
    ---
    input_a: f32[4] = [1.0, 2.0, 3.0, 4.0]
    input_b: i32 = 42
    scale: f32 = 2.5
    out_result: f32[4]
    local = 4, 1, 1
    global = (1, 1, 1)
    wave = 32
    ---
    s_load_b64 s[0:1], s[0:1], 0
    s_waitcnt lgkmcnt(0)
    "#;
    let path = write_temp(contents, "full_program");
    let mut program = program();
    let info = parse_file(&path, &mut program, Architecture::Rdna35).expect("parse file");

    assert_eq!(info.arguments.len(), 3);

    let input_a = &info.arguments[0];
    assert_eq!(input_a.name, "input_a");
    assert_eq!(input_a.len, 4);

    let input_b = &info.arguments[1];
    assert_eq!(input_b.name, "input_b");
    assert_eq!(input_b.len, 1);

    let scale = &info.arguments[2];
    assert_eq!(scale.name, "scale");
    assert_eq!(scale.len, 1);

    assert_eq!(info.output_arguments.len(), 1);
    let out_result = &info.output_arguments[0];
    assert_eq!(out_result.name, "out_result");
    assert_eq!(out_result.len, 4);

    // Verify launch config
    assert_eq!(info.local_launch_size, Dim3::new(4, 1, 1));
    assert_eq!(info.global_launch_size, Dim3::new(1, 1, 1));
    assert_eq!(info.wave_size, Some(WaveSize::Wave32));
  }

  #[test]
  fn test_dim3_parsing_both_formats() {
    // Format with parentheses
    let contents = r#"
    ---
    local = (2, 3, 4)
    global = (5, 6, 7)
    ---
    s_endpgm
    "#;
    let path = write_temp(contents, "dim3_parens");
    let mut prog1 = program();
    let info = parse_file(&path, &mut prog1, Architecture::Rdna35).expect("parse file");
    assert_eq!(info.local_launch_size, Dim3::new(2, 3, 4));
    assert_eq!(info.global_launch_size, Dim3::new(5, 6, 7));

    // Format without parentheses
    let contents = r#"
    ---
    local = 8, 9, 10
    global = 11, 12, 13
    ---
    s_endpgm
    "#;
    let path = write_temp(contents, "dim3_no_parens");
    let mut prog2 = program();
    let info = parse_file(&path, &mut prog2, Architecture::Rdna35).expect("parse file");
    assert_eq!(info.local_launch_size, Dim3::new(8, 9, 10));
    assert_eq!(info.global_launch_size, Dim3::new(11, 12, 13));

    // Extra whitespace
    let contents = r#"
    ---
    local = ( 1 , 2 , 3 )
    global =  4  ,  5  ,  6
    ---
    s_endpgm
    "#;
    let path = write_temp(contents, "dim3_whitespace");
    let mut prog3 = program();
    let info = parse_file(&path, &mut prog3, Architecture::Rdna35).expect("parse file");
    assert_eq!(info.local_launch_size, Dim3::new(1, 2, 3));
    assert_eq!(info.global_launch_size, Dim3::new(4, 5, 6));
  }

  #[test]
  fn test_dim3_parsing_errors() {
    // Too few values
    let contents = r#"
    ---
    local = 1, 2
    ---
    s_endpgm
    "#;
    let path = write_temp(contents, "dim3_too_few");
    let mut prog1 = program();
    let err = parse_file(&path, &mut prog1, Architecture::Rdna35).unwrap_err();
    assert!(err.contains("expected 3 values"));

    // Non-numeric value
    let contents = r#"
    ---
    local = 1, foo, 3
    ---
    s_endpgm
    "#;
    let path = write_temp(contents, "dim3_non_numeric");
    let mut prog2 = program();
    let err = parse_file(&path, &mut prog2, Architecture::Rdna35).unwrap_err();
    assert!(err.contains("line"));

    // Empty value
    let contents = r#"
    ---
    local = 1, , 3
    ---
    s_endpgm
    "#;
    let path = write_temp(contents, "dim3_empty");
    let mut prog3 = program();
    let err = parse_file(&path, &mut prog3, Architecture::Rdna35).unwrap_err();
    assert!(err.contains("expected 3 values") || err.contains("line"));

    // Just one value
    let contents = r#"
    ---
    local = 5
    ---
    s_endpgm
    "#;
    let path = write_temp(contents, "dim3_one_value");
    let mut prog4 = program();
    let err = parse_file(&path, &mut prog4, Architecture::Rdna35).unwrap_err();
    assert!(err.contains("expected 3 values"));
  }

  #[test]
  fn test_comment_handling() {
    // Comments with // in header
    let contents = r#"
    ---
    arg_a: i32 = 5 // this is a comment
    arg_b: f32 = 3.14 // another comment
    local = 1, 1, 1 // comment on dim3
    global = 1, 1, 1
    ---
    s_endpgm
    "#;
    let path = write_temp(contents, "comments_slash");
    let mut prog1 = program();
    let info = parse_file(&path, &mut prog1, Architecture::Rdna35).expect("parse file");
    assert_eq!(info.arguments.len(), 2);
    assert_eq!(info.arguments[0].name, "arg_a");
    assert_eq!(info.arguments[1].name, "arg_b");

    // Comments with ; in header and instructions
    let contents = r#"
    ---
    arg_c: i32 = 10 ; semicolon comment
    local = 2, 1, 1 ; another semicolon
    global = 1, 1, 1
    ---
    s_mov_b32 s0, 0 ; instruction comment
    v_mov_b32 v0, v0 ; another one
    "#;
    let path = write_temp(contents, "comments_semicolon");
    let mut prog2 = program();
    let info = parse_file(&path, &mut prog2, Architecture::Rdna35).expect("parse file");
    assert_eq!(info.arguments.len(), 1);
    assert_eq!(info.arguments[0].name, "arg_c");
    assert_eq!(info.instructions.len(), 2);
    assert_eq!(info.instructions[0].name, "s_mov_b32");
    assert_eq!(info.instructions[1].name, "v_mov_b32");

    // Mixed comment styles
    let contents = r#"
    ---
    arg_d: i32 = 15 // slash comment
    arg_e: i32 = 20 ; semicolon comment
    local = 1, 1, 1
    global = 1, 1, 1
    ---
    s_nop 0 // slash in instruction
    s_nop 0 ; semicolon in instruction
    "#;
    let path = write_temp(contents, "comments_mixed");
    let mut prog3 = program();
    let info = parse_file(&path, &mut prog3, Architecture::Rdna35).expect("parse file");
    assert_eq!(info.arguments.len(), 2);
    assert_eq!(info.instructions.len(), 2);
    assert_eq!(info.instructions[0].name, "s_nop");
    assert_eq!(info.instructions[1].name, "s_nop");
  }

  #[test]
  fn test_value_range_validation() {
    let mut prog1 = program();

    let arg = parse_argument("a", "u8 = 255", &mut prog1).unwrap();
    assert_eq!(arg.len, 1);

    let bytes = encode_values(&[Number::Int(256)], &spec("u8"));
    if let Ok(bytes) = bytes {
      assert_eq!(bytes[0], 0); // Truncated to u8
    }

    let mut prog2 = program();
    let arg = parse_argument("b", "i8 = -128", &mut prog2).unwrap();
    assert_eq!(arg.len, 1);

    let mut prog3 = program();
    let arg = parse_argument("c", "i8 = 127", &mut prog3).unwrap();
    assert_eq!(arg.len, 1);

    let bytes = encode_values(&[Number::Int(-129)], &spec("i8"));
    if let Ok(bytes) = bytes {
      assert_eq!(bytes[0] as i8, 127); // Truncated
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
