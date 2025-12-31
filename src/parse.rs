use std::fs;
use std::path::Path;
use std::str::FromStr;

use crate::sim::{generate_arange, generate_matrix};
use crate::{Dim3, Program, WaveSize};

#[derive(Clone, Debug)]
pub struct ArgInfo {
    pub name: String,
    pub type_name: String,
    pub shape: Vec<usize>,
    pub addr: u64,
    pub len: usize,
}

#[derive(Clone, Debug)]
pub struct ProgramInfo {
    pub instructions: Vec<String>,
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
}

impl TypeSpec {
    fn element_size(&self) -> usize {
        self.bits / 8
    }
}

#[derive(Clone, Debug)]
enum Number {
    Int(i64),
    Float(f64),
}

pub fn parse_file(file_path: &Path, program: &mut Program) -> Result<ProgramInfo, String> {
    let content = fs::read_to_string(file_path)
        .map_err(|e| format!("Failed to read file {}: {}", file_path.display(), e))?;

    let mut instructions = Vec::new();
    let mut arguments = Vec::new();
    let mut output_arguments = Vec::new();
    let mut local_launch_size = Dim3::new(1, 1, 1);
    let mut global_launch_size = Dim3::new(1, 1, 1);
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
                        local_launch_size = parse_dim3(value).map_err(|e| {
                            format!("line {}: invalid local size: {}", line_no + 1, e)
                        })?;
                    }
                    "global" => {
                        global_launch_size = parse_dim3(value).map_err(|e| {
                            format!("line {}: invalid global size: {}", line_no + 1, e)
                        })?;
                    }
                    "wave" => {
                        let val = value.parse::<u32>().map_err(|_| {
                            format!("line {}: invalid wave size '{}'", line_no + 1, value)
                        })?;
                        wave_size = match val {
                            32 => Some(WaveSize::Wave32),
                            64 => Some(WaveSize::Wave64),
                            _ => {
                                return Err(format!(
                                    "line {}: wave size must be 32 or 64",
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
            instructions.push(line.to_string());
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
    if let Some(idx) = line.find("//") {
        cut = cut.min(idx);
    }
    if let Some(idx) = line.find('#') {
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

fn parse_dim3(value: &str) -> Result<Dim3, String> {
    let trimmed = value.trim().trim_start_matches('(').trim_end_matches(')');
    Dim3::from_str(trimmed).map_err(|e| e.to_string())
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
    if is_float && bits != 32 && bits != 64 {
        return Err(format!("unsupported float type '{}'", value));
    }
    Ok(TypeSpec {
        name: lower,
        bits,
        is_float,
        is_signed,
    })
}

fn parse_initializer(
    value: &str,
    spec: &TypeSpec,
    expected_len: usize,
    shape: &[usize],
) -> Result<Vec<Number>, String> {
    let value = value.trim();
    let mut numbers = if let Some(inner) = value.strip_prefix('[').and_then(|v| v.strip_suffix(']')) {
        parse_number_list(inner)?
    } else if let Some(args) = value.strip_prefix("arange(").and_then(|v| v.strip_suffix(')')) {
        let args = parse_call_args(args)?;
        let (start, end, step) = match args.len() {
            1 => (0.0, args[0], 1.0),
            2 => (args[0], args[1], 1.0),
            3 => (args[0], args[1], args[2]),
            _ => return Err("arange expects 1-3 args".to_string()),
        };
        generate_arange(start, end, step)?.into_iter().map(Number::Float).collect()
    } else if let Some(args) = value.strip_prefix("matrix(").and_then(|v| v.strip_suffix(')')) {
        let args = parse_call_args(args)?;
        if args.len() < 2 || args.len() > 4 {
            return Err("matrix expects 2-4 args".to_string());
        }
        let rows = parse_usize_arg(args[0], "matrix rows")?;
        let cols = parse_usize_arg(args[1], "matrix cols")?;
        let start = if args.len() >= 3 { args[2] } else { 0.0 };
        let step = if args.len() >= 4 { args[3] } else { 1.0 };
        if !shape.is_empty() {
            let product = shape.iter().product::<usize>();
            if product != rows * cols {
                return Err("matrix shape does not match type shape".to_string());
            }
        }
        generate_matrix(rows, cols, start, step)
            .into_iter()
            .map(Number::Float)
            .collect()
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

    if !spec.is_float {
        for num in &mut numbers {
            *num = Number::Int(number_to_int(num)?);
        }
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

fn parse_call_args(value: &str) -> Result<Vec<f64>, String> {
    let mut out = Vec::new();
    for token in value.split(',') {
        let token = token.trim();
        if token.is_empty() {
            continue;
        }
        let num = parse_number(token)?;
        out.push(match num {
            Number::Int(v) => v as f64,
            Number::Float(v) => v,
        });
    }
    Ok(out)
}

fn parse_usize_arg(value: f64, label: &str) -> Result<usize, String> {
    if value.fract() != 0.0 || value < 0.0 {
        return Err(format!("{} must be an integer", label));
    }
    usize::try_from(value as i64).map_err(|_| format!("{} out of range", label))
}

fn parse_number(value: &str) -> Result<Number, String> {
    let value = value.trim();
    if value.contains('.') || value.contains('e') || value.contains('E') {
        let num = value
            .parse::<f64>()
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

fn number_to_int(value: &Number) -> Result<i64, String> {
    match *value {
        Number::Int(v) => Ok(v),
        Number::Float(v) => {
            if v.fract() != 0.0 {
                Err("integer value required".to_string())
            } else {
                Ok(v as i64)
            }
        }
    }
}

fn encode_values(values: &[Number], spec: &TypeSpec) -> Result<Vec<u8>, String> {
    let mut out = Vec::with_capacity(values.len() * spec.element_size());
    for value in values {
        if spec.is_float {
            let float_val = match *value {
                Number::Int(v) => v as f64,
                Number::Float(v) => v,
            };
            match spec.bits {
                32 => out.extend_from_slice(&(float_val as f32).to_le_bytes()),
                64 => out.extend_from_slice(&float_val.to_le_bytes()),
                _ => return Err(format!("unsupported float width {}", spec.bits)),
            }
        } else {
            let int_val = number_to_int(value)?;
            if spec.is_signed {
                match spec.bits {
                    8 => out.extend_from_slice(&(int_val as i8).to_le_bytes()),
                    16 => out.extend_from_slice(&(int_val as i16).to_le_bytes()),
                    32 => out.extend_from_slice(&(int_val as i32).to_le_bytes()),
                    64 => out.extend_from_slice(&(int_val as i64).to_le_bytes()),
                    _ => return Err(format!("unsupported int width {}", spec.bits)),
                }
            } else {
                if int_val < 0 {
                    return Err("unsigned value must be >= 0".to_string());
                }
                let uval = int_val as u64;
                match spec.bits {
                    8 => out.extend_from_slice(&(uval as u8).to_le_bytes()),
                    16 => out.extend_from_slice(&(uval as u16).to_le_bytes()),
                    32 => out.extend_from_slice(&(uval as u32).to_le_bytes()),
                    64 => out.extend_from_slice(&(uval as u64).to_le_bytes()),
                    _ => return Err(format!("unsupported int width {}", spec.bits)),
                }
            }
        }
    }
    Ok(out)
}
