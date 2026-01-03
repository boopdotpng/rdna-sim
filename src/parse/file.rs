use std::fs;
use std::path::Path;
use std::str::FromStr;

use crate::decode::{decode_instruction, format_decode_error};
use crate::isa::types::InstructionCommonDef;
use crate::parse_instruction::parse_instruction;
use crate::sim::{DecodedInst, DecodedOperand, DualInst};
use crate::{Architecture, Dim3, Program, WaveSize};

use super::args::{parse_argument, ProgramInfo};

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
      if let Some((left, right)) = split_vopd_line(line)
        .map_err(|e| format!("line {}: {}", line_no + 1, e))? {
        let left_parsed = parse_instruction(left)
          .map_err(|e| format!("line {}: {}", line_no + 1, e))?;
        let right_parsed = parse_instruction(right)
          .map_err(|e| format!("line {}: {}", line_no + 1, e))?;

        if !is_v_dual(&left_parsed.name) || !is_v_dual(&right_parsed.name) {
          return Err(format!(
            "line {}: vopd pair requires v_dual_* on both sides",
            line_no + 1
          ));
        }

        let left_def = lookup_instruction_def(&left_parsed.name, arch)
          .ok_or_else(|| format!("line {}: unknown instruction '{}'",
            line_no + 1, left_parsed.name))?;
        let right_def = lookup_instruction_def(&right_parsed.name, arch)
          .ok_or_else(|| format!("line {}: unknown instruction '{}'",
            line_no + 1, right_parsed.name))?;

        let left_decoded = decode_instruction(&left_parsed, left_def, line_no + 1)
          .map_err(|e| format_decode_error(e))?;
        let right_decoded = decode_instruction(&right_parsed, right_def, line_no + 1)
          .map_err(|e| format_decode_error(e))?;

        validate_vopd_pair(&left_decoded, &right_decoded, line_no + 1)?;
        let dual = DualInst {
          name: right_decoded.name,
          def: right_decoded.def,
          operands: right_decoded.operands,
        };

        let mut combined = left_decoded;
        combined.dual = Some(dual);
        instructions.push(combined);
      } else {
        let parsed = parse_instruction(line)
          .map_err(|e| format!("line {}: {}", line_no + 1, e))?;

        if is_v_dual(&parsed.name) {
          return Err(format!(
            "line {}: v_dual_* must be paired with '::'",
            line_no + 1
          ));
        }

        let def = lookup_instruction_def(&parsed.name, arch)
          .ok_or_else(|| format!("line {}: unknown instruction '{}'",
            line_no + 1, parsed.name))?;

        let decoded = decode_instruction(&parsed, def, line_no + 1)
          .map_err(|e| format_decode_error(e))?;

        instructions.push(decoded);
      }
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

fn lookup_instruction_def(name: &str, arch: Architecture) -> Option<&'static InstructionCommonDef> {
  if let Some(common_def) = crate::isa::base::lookup_common_normalized(name) {
    return Some(common_def);
  }

  match arch {
    Architecture::Rdna35 => crate::isa::rdna35::lookup_common_def(name),
  }
}

fn split_vopd_line(line: &str) -> Result<Option<(&str, &str)>, String> {
  let Some((left, right)) = line.split_once("::") else {
    return Ok(None);
  };
  if right.contains("::") {
    return Err("vopd supports a single '::' separator".to_string());
  }
  let left = left.trim();
  let right = right.trim();
  if left.is_empty() {
    return Err("vopd missing instruction before '::'".to_string());
  }
  if right.is_empty() {
    return Err("vopd missing instruction after '::'".to_string());
  }
  Ok(Some((left, right)))
}

fn is_v_dual(name: &str) -> bool {
  name.starts_with("v_dual_")
}

#[derive(Copy, Clone, Debug, PartialEq)]
enum LiteralValue {
  U32(u32),
  I32(i32),
  F32(u32),
}

fn literal_value(op: &DecodedOperand) -> Option<LiteralValue> {
  match op {
    DecodedOperand::ImmU32(v) => Some(LiteralValue::U32(*v)),
    DecodedOperand::ImmI32(v) => Some(LiteralValue::I32(*v)),
    DecodedOperand::ImmF32(v) => Some(LiteralValue::F32(v.to_bits())),
    DecodedOperand::Negate(inner) => literal_value(inner),
    DecodedOperand::Abs(inner) => literal_value(inner),
    _ => None,
  }
}

fn literal_from_sources(
  operands: &[&DecodedOperand],
  half_label: &str,
  line_num: usize,
) -> Result<Option<LiteralValue>, String> {
  let mut literal = None;
  for operand in operands {
    if let Some(found) = literal_value(*operand) {
      if literal.is_some() {
        return Err(format!(
          "line {}: vopd {} half uses more than one literal constant",
          line_num, half_label
        ));
      }
      literal = Some(found);
    }
  }
  Ok(literal)
}

fn vgpr_index(op: &DecodedOperand) -> Option<u16> {
  match op {
    DecodedOperand::Vgpr(idx) => Some(*idx),
    _ => None,
  }
}

fn validate_vopd_pair(
  left: &DecodedInst,
  right: &DecodedInst,
  line_num: usize,
) -> Result<(), String> {
  let left_ops: Vec<&DecodedOperand> = left.operands
    .iter()
    .filter(|op| !matches!(op, DecodedOperand::Flag(_)))
    .collect();
  let right_ops: Vec<&DecodedOperand> = right.operands
    .iter()
    .filter(|op| !matches!(op, DecodedOperand::Flag(_)))
    .collect();

  let left_dst = left_ops
    .get(0)
    .and_then(|op| vgpr_index(op))
    .ok_or_else(|| format!(
      "line {}: vopd left half destination must be a vgpr",
      line_num
    ))?;
  let right_dst = right_ops
    .get(0)
    .and_then(|op| vgpr_index(op))
    .ok_or_else(|| format!(
      "line {}: vopd right half destination must be a vgpr",
      line_num
    ))?;

  if (left_dst & 1) == (right_dst & 1) {
    return Err(format!(
      "line {}: vopd vdst parity requires one even and one odd (got v{} and v{})",
      line_num, left_dst, right_dst
    ));
  }

  let left_sources = if left_ops.len() > 1 { &left_ops[1..] } else { &[] };
  let right_sources = if right_ops.len() > 1 { &right_ops[1..] } else { &[] };
  let left_literal = literal_from_sources(left_sources, "left", line_num)?;
  let right_literal = literal_from_sources(right_sources, "right", line_num)?;
  if let (Some(left_val), Some(right_val)) = (left_literal, right_literal) {
    if left_val != right_val {
      return Err(format!(
        "line {}: vopd literal must match across halves",
        line_num
      ));
    }
  }

  let left_src0 = left_ops.get(1).and_then(|op| vgpr_index(op));
  let right_src0 = right_ops.get(1).and_then(|op| vgpr_index(op));
  if let (Some(left_idx), Some(right_idx)) = (left_src0, right_src0) {
    if (left_idx % 4) == (right_idx % 4) {
      return Err(format!(
        "line {}: vopd src0 bank conflict between v{} and v{}",
        line_num, left_idx, right_idx
      ));
    }
  }

  let left_src1 = left_ops.get(2).and_then(|op| vgpr_index(op));
  let right_src1 = right_ops.get(2).and_then(|op| vgpr_index(op));
  if let (Some(left_idx), Some(right_idx)) = (left_src1, right_src1) {
    if (left_idx % 4) == (right_idx % 4) {
      return Err(format!(
        "line {}: vopd src1 bank conflict between v{} and v{}",
        line_num, left_idx, right_idx
      ));
    }
  }

  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::parse::test_support::{program, write_temp};

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

    assert_eq!(info.local_launch_size, Dim3::new(4, 1, 1));
    assert_eq!(info.global_launch_size, Dim3::new(1, 1, 1));
    assert_eq!(info.wave_size, Some(WaveSize::Wave32));
  }

  #[test]
  fn test_dim3_parsing_both_formats() {
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
  fn parse_vopd_pair() {
    let contents = r#"
    v_dual_add_f32 v0, v1, v2 :: v_dual_mul_f32 v1, v3, v4
    "#;
    let path = write_temp(contents, "vopd_pair");
    let mut program = program();
    let info = parse_file(&path, &mut program, Architecture::Rdna35).expect("parse file");
    assert_eq!(info.instructions.len(), 1);
    let inst = &info.instructions[0];
    assert_eq!(inst.name, "v_dual_add_f32");
    let dual = inst.dual.as_ref().expect("dual");
    assert_eq!(dual.name, "v_dual_mul_f32");
  }

  #[test]
  fn parse_vopd_requires_dual() {
    let contents = r#"
    v_dual_add_f32 v0, v1, v2
    "#;
    let path = write_temp(contents, "vopd_missing");
    let mut program = program();
    let err = parse_file(&path, &mut program, Architecture::Rdna35).unwrap_err();
    assert!(err.contains("v_dual_* must be paired"), "{}", err);
  }

  #[test]
  fn parse_vopd_requires_dual_on_both_sides() {
    let contents = r#"
    v_add_f32 v0, v1, v2 :: v_dual_add_f32 v1, v2, v3
    "#;
    let path = write_temp(contents, "vopd_non_dual");
    let mut program = program();
    let err = parse_file(&path, &mut program, Architecture::Rdna35).unwrap_err();
    assert!(err.contains("vopd pair requires v_dual_*"), "{}", err);
  }

  #[test]
  fn parse_vopd_rejects_missing_right_half() {
    let contents = r#"
    v_dual_add_f32 v0, v1, v2 ::
    "#;
    let path = write_temp(contents, "vopd_missing_right");
    let mut program = program();
    let err = parse_file(&path, &mut program, Architecture::Rdna35).unwrap_err();
    assert!(err.contains("missing instruction after"), "{}", err);
  }

  #[test]
  fn parse_vopd_rejects_vdst_parity_conflict() {
    let contents = r#"
    v_dual_add_f32 v0, v1, v2 :: v_dual_mul_f32 v2, v3, v4
    "#;
    let path = write_temp(contents, "vopd_vdst_parity");
    let mut program = program();
    let err = parse_file(&path, &mut program, Architecture::Rdna35).unwrap_err();
    assert!(err.contains("vdst parity"), "{}", err);
  }

  #[test]
  fn parse_vopd_rejects_literal_mismatch() {
    let contents = r#"
    v_dual_add_f32 v0, 1.0, v2 :: v_dual_add_f32 v1, 2.0, v3
    "#;
    let path = write_temp(contents, "vopd_literal_mismatch");
    let mut program = program();
    let err = parse_file(&path, &mut program, Architecture::Rdna35).unwrap_err();
    assert!(err.contains("literal must match"), "{}", err);
  }

  #[test]
  fn parse_vopd_rejects_src0_bank_conflict() {
    let contents = r#"
    v_dual_add_f32 v0, v1, v2 :: v_dual_add_f32 v1, v5, v7
    "#;
    let path = write_temp(contents, "vopd_src0_bank");
    let mut program = program();
    let err = parse_file(&path, &mut program, Architecture::Rdna35).unwrap_err();
    assert!(err.contains("src0 bank conflict"), "{}", err);
  }
}
