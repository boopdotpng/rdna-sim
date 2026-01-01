// Instruction decoding and validation
// Converts ParsedInstruction to DecodedInst with strict validation

use crate::isa::types::{ArgKind, ArgSpec, InstructionCommonDef};
use crate::parse_instruction::{Operand, ParsedInstruction};
use crate::sim::{DecodedInst, DecodedOperand};

#[derive(Debug)]
pub enum DecodeError {
    UnknownInstruction(String, usize),              // name, line
    OperandCountMismatch {
        expected: usize,
        got: usize,
        instruction: String,
        line: usize
    },
    OperandTypeMismatch {
        expected: ArgKind,
        got: String,
        operand_index: usize,
        instruction: String,
        line: usize
    },
    InvalidOperand(String, usize),                  // error, line
    ModifierNotSupported {
        modifier: String,  // "negation" or "absolute value"
        instruction: String,
        line: usize,
    },
}

fn validate_modifier(
    operand: &Operand,
    def: &'static InstructionCommonDef,
    inst_name: &str,
    line_num: usize,
) -> Result<(), DecodeError> {
    match operand {
        Operand::Negate(inner) => {
            // Check if instruction supports negation
            if !def.supports_neg {
                return Err(DecodeError::ModifierNotSupported {
                    modifier: "negation".to_string(),
                    instruction: inst_name.to_string(),
                    line: line_num,
                });
            }

            // Recursively validate inner operand
            validate_modifier(inner, def, inst_name, line_num)?;
        }

        Operand::Abs(inner) => {
            // Check if instruction supports absolute value
            if !def.supports_abs {
                return Err(DecodeError::ModifierNotSupported {
                    modifier: "absolute value".to_string(),
                    instruction: inst_name.to_string(),
                    line: line_num,
                });
            }

            // Recursively validate inner operand
            validate_modifier(inner, def, inst_name, line_num)?;
        }

        _ => {}  // Non-modifier operands are ok
    }
    Ok(())
}

pub fn decode_instruction(
    parsed: &ParsedInstruction,
    def: &'static InstructionCommonDef,
    line_num: usize,
) -> Result<DecodedInst, DecodeError> {
    // Validate all modifiers before processing operands
    for operand in &parsed.operands {
        validate_modifier(operand, def, &parsed.name, line_num)?;
    }

    // Validate operand count
    // Note: Flags are permissive and can appear anywhere, so we need to filter them
    let non_flag_operands: Vec<_> = parsed.operands.iter()
        .filter(|op| !matches!(op, Operand::Flag(_)))
        .collect();

    if non_flag_operands.len() != def.args.len() {
        return Err(DecodeError::OperandCountMismatch {
            expected: def.args.len(),
            got: non_flag_operands.len(),
            instruction: parsed.name.clone(),
            line: line_num,
        });
    }

    // Validate and convert each operand
    let mut decoded_operands = Vec::new();
    let mut non_flag_idx = 0;

    for operand in &parsed.operands {
        match operand {
            Operand::Flag(_) => {
                // Flags are always allowed, convert directly
                decoded_operands.push(convert_operand(operand)?);
            }
            _ => {
                // Non-flag operands must match ArgSpec
                if non_flag_idx >= def.args.len() {
                    return Err(DecodeError::InvalidOperand(
                        format!("unexpected operand at position {}", decoded_operands.len()),
                        line_num
                    ));
                }

                let spec = &def.args[non_flag_idx];
                let decoded = validate_and_convert_operand(
                    operand,
                    spec,
                    non_flag_idx,
                    &parsed.name,
                    line_num,
                )?;
                decoded_operands.push(decoded);
                non_flag_idx += 1;
            }
        }
    }

    Ok(DecodedInst {
        name: parsed.name.clone(),
        def,
        line_num,
        operands: decoded_operands,
    })
}

fn validate_and_convert_operand(
    operand: &Operand,
    spec: &ArgSpec,
    operand_idx: usize,
    inst_name: &str,
    line_num: usize,
) -> Result<DecodedOperand, DecodeError> {
    // Handle modifiers (Negate, Abs) - unwrap and validate inner operand
    let operand = match operand {
        Operand::Negate(inner) => {
            let decoded_inner = validate_and_convert_operand(inner, spec, operand_idx, inst_name, line_num)?;
            return Ok(DecodedOperand::Negate(Box::new(decoded_inner)));
        }
        Operand::Abs(inner) => {
            let decoded_inner = validate_and_convert_operand(inner, spec, operand_idx, inst_name, line_num)?;
            return Ok(DecodedOperand::Abs(Box::new(decoded_inner)));
        }
        _ => operand,
    };

    // Validate operand type matches ArgKind
    let matches = match (operand, spec.kind) {
        // Sgpr operands
        (Operand::Sgpr(_), ArgKind::Sgpr) => true,
        (Operand::SgprRange(_, _), ArgKind::Sgpr) => true,
        (Operand::SpecialReg(_), ArgKind::Sgpr) => true,

        // Vgpr operands
        (Operand::Vgpr(_), ArgKind::Vgpr) => true,
        (Operand::VgprRange(_, _), ArgKind::Vgpr) => true,

        // Special registers
        (Operand::SpecialReg(_), ArgKind::Special) => true,

        // Immediate operands
        (Operand::ImmU32(_), ArgKind::Imm) => true,
        (Operand::ImmI32(_), ArgKind::Imm) => true,
        (Operand::ImmF32(_), ArgKind::Imm) => true,

        // SgprOrImm accepts scalar registers or immediates (for s_* instructions)
        (Operand::Sgpr(_), ArgKind::SgprOrImm) => true,
        (Operand::SgprRange(_, _), ArgKind::SgprOrImm) => true,
        (Operand::SpecialReg(_), ArgKind::SgprOrImm) => true,
        (Operand::ImmU32(_), ArgKind::SgprOrImm) => true,
        (Operand::ImmI32(_), ArgKind::SgprOrImm) => true,
        (Operand::ImmF32(_), ArgKind::SgprOrImm) => true,

        // VgprOrImm accepts vector registers or immediates (for v_* instructions)
        (Operand::Vgpr(_), ArgKind::VgprOrImm) => true,
        (Operand::VgprRange(_, _), ArgKind::VgprOrImm) => true,
        (Operand::ImmU32(_), ArgKind::VgprOrImm) => true,
        (Operand::ImmI32(_), ArgKind::VgprOrImm) => true,
        (Operand::ImmF32(_), ArgKind::VgprOrImm) => true,

        // Memory operands (offsets allowed for Mem kind)
        (Operand::Offset(_), ArgKind::Mem) => true,

        // Labels
        // Note: We don't have Label parsing yet, so this is future-proofing
        (_, ArgKind::Label) => false,

        // Unknown operands always fail
        (_, ArgKind::Unknown) => false,

        // Any mismatch
        _ => false,
    };

    if !matches {
        return Err(DecodeError::OperandTypeMismatch {
            expected: spec.kind,
            got: format!("{:?}", operand),
            operand_index: operand_idx,
            instruction: inst_name.to_string(),
            line: line_num,
        });
    }

    // Convert the operand (type is already validated)
    convert_operand(operand)
}

fn convert_operand(operand: &Operand) -> Result<DecodedOperand, DecodeError> {
    Ok(match operand {
        Operand::Sgpr(idx) => DecodedOperand::Sgpr(*idx),
        Operand::SgprRange(start, end) => DecodedOperand::SgprRange(*start, *end),
        Operand::Vgpr(idx) => DecodedOperand::Vgpr(*idx),
        Operand::VgprRange(start, end) => DecodedOperand::VgprRange(*start, *end),
        Operand::SpecialReg(reg) => DecodedOperand::SpecialReg(reg.clone()),
        Operand::ImmU32(val) => DecodedOperand::ImmU32(*val),
        Operand::ImmI32(val) => DecodedOperand::ImmI32(*val),
        Operand::ImmF32(val) => DecodedOperand::ImmF32(*val),
        Operand::Offset(val) => DecodedOperand::Offset(*val),
        Operand::Flag(name) => DecodedOperand::Flag(name.clone()),
        Operand::Negate(inner) => {
            DecodedOperand::Negate(Box::new(convert_operand(inner)?))
        }
        Operand::Abs(inner) => {
            DecodedOperand::Abs(Box::new(convert_operand(inner)?))
        }
    })
}

pub fn format_decode_error(err: DecodeError) -> String {
    match err {
        DecodeError::UnknownInstruction(name, line) => {
            format!("line {}: unknown instruction '{}'", line, name)
        }
        DecodeError::OperandCountMismatch { expected, got, instruction, line } => {
            format!("line {}: instruction '{}' expects {} operands, got {}",
                    line, instruction, expected, got)
        }
        DecodeError::OperandTypeMismatch { expected, got, operand_index, instruction, line } => {
            format!("line {}: instruction '{}' operand {} expects {:?}, got {}",
                    line, instruction, operand_index + 1, expected, got)
        }
        DecodeError::InvalidOperand(msg, line) => {
            format!("line {}: {}", line, msg)
        }
        DecodeError::ModifierNotSupported { modifier, instruction, line } => {
            format!("line {}: {} modifier not supported on instruction '{}'",
                    line, modifier, instruction)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse_instruction::parse_instruction;
    use crate::Architecture;

    // Helper to lookup instruction definition
    fn lookup_inst_def(name: &str) -> &'static InstructionCommonDef {
        // Try base ISA first
        if let Some(def) = crate::isa::base::lookup_common_normalized(name) {
            return def;
        }
        // Then arch-specific (RDNA3.5)
        crate::isa::rdna35::lookup_common_def(name)
            .expect(&format!("instruction {} not found", name))
    }

    #[test]
    fn test_reject_vgpr_in_scalar_instruction() {
        // s_mov_b32 s0, v2 should be rejected (VGPR in scalar instruction)
        let parsed = parse_instruction("s_mov_b32 s0, v2").expect("parse failed");
        let def = lookup_inst_def("s_mov_b32");

        let result = decode_instruction(&parsed, def, 1);
        assert!(result.is_err(), "Expected error when using VGPR in scalar instruction");

        match result.unwrap_err() {
            DecodeError::OperandTypeMismatch { expected, .. } => {
                assert_eq!(expected, ArgKind::SgprOrImm);
            }
            other => panic!("Expected OperandTypeMismatch, got {:?}", other),
        }
    }

    #[test]
    fn test_reject_sgpr_in_vector_instruction() {
        // v_mov_b32 v0, s2 should be rejected (SGPR in vector instruction)
        let parsed = parse_instruction("v_mov_b32 v0, s2").expect("parse failed");
        let def = lookup_inst_def("v_mov_b32");

        let result = decode_instruction(&parsed, def, 1);
        assert!(result.is_err(), "Expected error when using SGPR in vector instruction");

        match result.unwrap_err() {
            DecodeError::OperandTypeMismatch { expected, .. } => {
                assert_eq!(expected, ArgKind::VgprOrImm);
            }
            other => panic!("Expected OperandTypeMismatch, got {:?}", other),
        }
    }

    #[test]
    fn test_accept_immediate_in_scalar_instruction() {
        // s_mov_b32 s0, 0 should be accepted
        let parsed = parse_instruction("s_mov_b32 s0, 0").expect("parse failed");
        let def = lookup_inst_def("s_mov_b32");

        let result = decode_instruction(&parsed, def, 1);
        assert!(result.is_ok(), "Expected success with immediate in scalar instruction");
    }

    #[test]
    fn test_accept_immediate_in_vector_instruction() {
        // v_mov_b32 v0, 0 should be accepted
        let parsed = parse_instruction("v_mov_b32 v0, 0").expect("parse failed");
        let def = lookup_inst_def("v_mov_b32");

        let result = decode_instruction(&parsed, def, 1);
        assert!(result.is_ok(), "Expected success with immediate in vector instruction");
    }

    #[test]
    fn test_accept_correct_register_types() {
        // s_mov_b32 s0, s1 should be accepted
        let parsed = parse_instruction("s_mov_b32 s0, s1").expect("parse failed");
        let def = lookup_inst_def("s_mov_b32");
        assert!(decode_instruction(&parsed, def, 1).is_ok());

        // v_mov_b32 v0, v1 should be accepted
        let parsed = parse_instruction("v_mov_b32 v0, v1").expect("parse failed");
        let def = lookup_inst_def("v_mov_b32");
        assert!(decode_instruction(&parsed, def, 1).is_ok());
    }

    // Modifier validation tests
    #[test]
    fn test_reject_modifiers_on_scalar_instructions() {
        // s_mov_b32 s0, -s1 should be rejected (scalar instruction doesn't support modifiers)
        let parsed = parse_instruction("s_mov_b32 s0, -s1").expect("parse failed");
        let def = lookup_inst_def("s_mov_b32");

        let result = decode_instruction(&parsed, def, 1);
        assert!(result.is_err(), "Expected error for negation on scalar instruction");

        match result.unwrap_err() {
            DecodeError::ModifierNotSupported { modifier, .. } => {
                assert_eq!(modifier, "negation");
            }
            other => panic!("Expected ModifierNotSupported, got {:?}", other),
        }
    }

    #[test]
    fn test_reject_abs_on_scalar_instruction() {
        // s_mov_b32 s0, |s1| should be rejected
        let parsed = parse_instruction("s_mov_b32 s0, |s1|").expect("parse failed");
        let def = lookup_inst_def("s_mov_b32");

        let result = decode_instruction(&parsed, def, 1);
        assert!(result.is_err(), "Expected error for abs on scalar instruction");

        match result.unwrap_err() {
            DecodeError::ModifierNotSupported { modifier, .. } => {
                assert_eq!(modifier, "absolute value");
            }
            other => panic!("Expected ModifierNotSupported, got {:?}", other),
        }
    }

    #[test]
    fn test_accept_modifiers_on_fp_vector() {
        // v_add_f32 v0, -v1, |v2| should work (VOP3 encoding supports modifiers)
        let parsed = parse_instruction("v_add_f32 v0, -v1, |v2|").expect("parse failed");
        let def = lookup_inst_def("v_add_f32");

        let result = decode_instruction(&parsed, def, 1);
        assert!(result.is_ok(), "Expected success with modifiers on FP vector instruction");
    }

    #[test]
    fn test_accept_combined_neg_abs() {
        // v_mul_f32 v0, -|v1|, v2 should work (abs then negate)
        let parsed = parse_instruction("v_mul_f32 v0, -|v1|, v2").expect("parse failed");
        let def = lookup_inst_def("v_mul_f32");

        let result = decode_instruction(&parsed, def, 1);
        assert!(result.is_ok(), "Expected success with combined modifiers");
    }

    #[test]
    fn test_modifiers_on_scalar_reg_in_vector_inst() {
        // v_add_f32 v0, -1.0, v1 should work (immediate with modifier, data type compatible)
        let parsed = parse_instruction("v_add_f32 v0, -1.0, v1").expect("parse failed");
        let def = lookup_inst_def("v_add_f32");

        let result = decode_instruction(&parsed, def, 1);
        assert!(result.is_ok(), "Expected success with modifier on immediate");
    }
}
