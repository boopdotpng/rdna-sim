// Instruction parsing for RDNA assembly
// Parses individual assembly instructions into structured data

#[derive(Clone, Debug, PartialEq)]
pub struct ParsedInstruction {
    pub name: String,
    pub operands: Vec<Operand>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Operand {
    // Register operands
    Sgpr(u16),                    // s[2]
    Vgpr(u16),                    // v[2]
    SgprRange(u16, u16),          // s[1:4] - start and end (inclusive)
    VgprRange(u16, u16),          // v[1:4]

    // Special registers
    SpecialReg(SpecialRegister),

    // Operand modifiers
    Negate(Box<Operand>),         // -v0
    Abs(Box<Operand>),            // |v0|

    // Immediate values
    ImmU32(u32),                  // 42, 0x2a, 0b1010
    ImmI32(i32),                  // -42
    ImmF32(f32),                  // 1.0, -2.5

    // Memory operands
    Offset(u32),                  // offset:256

    // Cache policy / flags (treated as flag operands)
    Flag(String),                 // glc, slc, nt, offen, idxen
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum SpecialRegister {
    Vcc,
    VccLo,
    VccHi,
    Exec,
    ExecLo,
    ExecHi,
    M0,
    Null,
    Scc,
}

pub fn parse_instruction(line: &str) -> Result<ParsedInstruction, String> {
    let line = line.trim();
    if line.is_empty() {
        return Err("empty instruction".to_string());
    }

    // Split instruction name from operands
    let (name_part, operands_part) = split_instruction(line)?;
    let name = normalize_instruction_name(name_part);

    // Handle special instructions
    if name == "s_waitcnt" {
        return parse_waitcnt(operands_part);
    }
    if name == "s_sendmsg" {
        return parse_sendmsg(operands_part);
    }

    // Parse regular operands
    let operands = if operands_part.is_empty() {
        Vec::new()
    } else {
        parse_operands(operands_part)?
    };

    Ok(ParsedInstruction { name, operands })
}

fn split_instruction(line: &str) -> Result<(&str, &str), String> {
    // Find first whitespace or comma to split instruction name from operands
    let mut split_pos = line.len();
    for (idx, ch) in line.char_indices() {
        if ch.is_whitespace() || ch == ',' {
            split_pos = idx;
            break;
        }
    }

    let name = &line[..split_pos];
    let rest = line[split_pos..].trim();

    Ok((name, rest))
}

fn normalize_instruction_name(name: &str) -> String {
    let lower = name.to_ascii_lowercase();

    // Strip encoding suffixes (_e32, _e64, etc.)
    if let Some(pos) = lower.rfind("_e") {
        if let Some(suffix) = lower.get(pos + 2..) {
            if suffix.chars().all(|c| c.is_ascii_digit()) {
                return lower[..pos].to_string();
            }
        }
    }

    lower
}

fn parse_operands(operands_str: &str) -> Result<Vec<Operand>, String> {
    let mut operands = Vec::new();

    // Split by commas, respecting brackets and abs modifiers
    let mut current = String::new();
    let mut depth = 0;
    let mut in_abs = false;

    for ch in operands_str.chars() {
        match ch {
            '[' => {
                depth += 1;
                current.push(ch);
            }
            ']' => {
                depth -= 1;
                current.push(ch);
            }
            '|' => {
                in_abs = !in_abs;
                current.push(ch);
            }
            ',' if depth == 0 && !in_abs => {
                let token = current.trim();
                if !token.is_empty() {
                    operands.push(parse_operand(token)?);
                }
                current.clear();
            }
            _ => {
                current.push(ch);
            }
        }
    }

    // Process the last token (may have space-separated items like flags)
    let remaining = current.trim();
    if !remaining.is_empty() {
        // Check if this looks like it has space-separated tokens (flags, offset, etc.)
        if remaining.contains(char::is_whitespace) {
            // Split on whitespace for the last group of operands
            for token in remaining.split_whitespace() {
                operands.push(parse_operand(token)?);
            }
        } else {
            operands.push(parse_operand(remaining)?);
        }
    }

    Ok(operands)
}

fn parse_operand(op: &str) -> Result<Operand, String> {
    let op = op.trim();

    // Check for offset:N
    if let Some(val_str) = op.strip_prefix("offset:") {
        let val = parse_number_u32(val_str)?;
        return Ok(Operand::Offset(val));
    }

    // Check for operand modifiers
    if op.starts_with('-') && op.len() > 1 {
        let inner = &op[1..];
        // Could be negative immediate or negated operand
        if inner.starts_with('|') || inner.starts_with('v') || inner.starts_with('s') {
            let inner_operand = parse_operand(inner)?;
            // Reject negation on register ranges
            if matches!(inner_operand, Operand::SgprRange(_, _) | Operand::VgprRange(_, _)) {
                return Err(format!("cannot apply negation modifier to register range: {}", op));
            }
            return Ok(Operand::Negate(Box::new(inner_operand)));
        }
        // Otherwise it's a negative immediate
        return parse_immediate(op);
    }

    if op.starts_with('|') && op.ends_with('|') && op.len() > 2 {
        let inner = &op[1..op.len() - 1];
        let inner_operand = parse_operand(inner)?;
        // Reject absolute value on register ranges
        if matches!(inner_operand, Operand::SgprRange(_, _) | Operand::VgprRange(_, _)) {
            return Err(format!("cannot apply absolute value modifier to register range: {}", op));
        }
        return Ok(Operand::Abs(Box::new(inner_operand)));
    }

    // Check for special registers first (before generic register check)
    if let Some(special) = parse_special_register(op) {
        return Ok(Operand::SpecialReg(special));
    }

    // Check for registers (must have digit or bracket after prefix)
    if op.starts_with('s') && op.len() > 1 {
        let rest = &op[1..];
        if rest.starts_with('[') || rest.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false) {
            return parse_register(op, true);
        }
    }

    if op.starts_with('v') && op.len() > 1 {
        let rest = &op[1..];
        if rest.starts_with('[') || rest.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false) {
            return parse_register(op, false);
        }
    }

    // Detect malformed register-like tokens and error
    if (op == "s" || op == "v") ||
       (op.starts_with("s[") && !op.ends_with(']')) ||
       (op.starts_with("v[") && !op.ends_with(']')) {
        return Err(format!("malformed register: {}", op));
    }

    // Check for immediate values
    if op.chars().next().map(|c| c.is_ascii_digit() || c == '-').unwrap_or(false)
        || op.starts_with("0x") || op.starts_with("0b") {
        return parse_immediate(op);
    }

    // Otherwise it's a flag
    Ok(Operand::Flag(op.to_ascii_lowercase()))
}

fn parse_register(op: &str, is_sgpr: bool) -> Result<Operand, String> {
    let prefix = if is_sgpr { 's' } else { 'v' };
    let rest = &op[1..];

    if rest.starts_with('[') && rest.ends_with(']') {
        let inner = &rest[1..rest.len() - 1].trim();

        // Check for empty brackets
        if inner.is_empty() {
            return Err(format!("empty register brackets: {}", op));
        }

        // Check for range: s[0:1]
        if let Some(colon_pos) = inner.find(':') {
            let start_str = inner[..colon_pos].trim();
            let end_str = inner[colon_pos + 1..].trim();

            // Check for incomplete ranges
            if start_str.is_empty() || end_str.is_empty() {
                return Err(format!("incomplete register range: {}", op));
            }

            let start = start_str.parse::<u16>()
                .map_err(|_| format!("invalid register range start: {}", start_str))?;
            let end = end_str.parse::<u16>()
                .map_err(|_| format!("invalid register range end: {}", end_str))?;

            if end < start {
                return Err(format!("invalid register range: end ({}) < start ({})", end, start));
            }

            return Ok(if is_sgpr {
                Operand::SgprRange(start, end)
            } else {
                Operand::VgprRange(start, end)
            });
        } else {
            // Single register in brackets: s[0]
            let idx = inner.parse::<u16>()
                .map_err(|_| format!("invalid register index: {}", inner))?;
            return Ok(if is_sgpr {
                Operand::Sgpr(idx)
            } else {
                Operand::Vgpr(idx)
            });
        }
    } else {
        // Simple register: s0
        let idx = rest.parse::<u16>()
            .map_err(|_| format!("invalid {} register: {}", prefix, op))?;
        return Ok(if is_sgpr {
            Operand::Sgpr(idx)
        } else {
            Operand::Vgpr(idx)
        });
    }
}

fn parse_special_register(op: &str) -> Option<SpecialRegister> {
    let lower = op.to_ascii_lowercase();
    match lower.as_str() {
        "vcc" => Some(SpecialRegister::Vcc),
        "vcc_lo" => Some(SpecialRegister::VccLo),
        "vcc_hi" => Some(SpecialRegister::VccHi),
        "exec" => Some(SpecialRegister::Exec),
        "exec_lo" => Some(SpecialRegister::ExecLo),
        "exec_hi" => Some(SpecialRegister::ExecHi),
        "m0" => Some(SpecialRegister::M0),
        "null" => Some(SpecialRegister::Null),
        "scc" => Some(SpecialRegister::Scc),
        _ => None,
    }
}

fn parse_immediate(op: &str) -> Result<Operand, String> {
    // Parse as integer first (to handle hex/binary which may contain 'E')
    let (sign, rest) = if let Some(stripped) = op.strip_prefix('-') {
        (-1i64, stripped)
    } else {
        (1i64, op)
    };

    // Check for hex/binary first
    if rest.starts_with("0x") || rest.starts_with("0b") {
        let parsed = if let Some(hex) = rest.strip_prefix("0x") {
            i64::from_str_radix(hex, 16)
                .map_err(|_| format!("invalid hex immediate: {}", op))?
        } else if let Some(bin) = rest.strip_prefix("0b") {
            i64::from_str_radix(bin, 2)
                .map_err(|_| format!("invalid binary immediate: {}", op))?
        } else {
            unreachable!()
        };

        let value = sign * parsed;
        return if sign < 0 {
            Ok(Operand::ImmI32(value as i32))
        } else {
            Ok(Operand::ImmU32(value as u32))
        };
    }

    // Check if it's a float (contains '.' or 'e'/'E')
    if op.contains('.') || op.contains('e') || op.contains('E') {
        let val = op.parse::<f32>()
            .map_err(|_| format!("invalid float immediate: {}", op))?;
        return Ok(Operand::ImmF32(val));
    }

    // Parse as decimal integer
    let parsed = rest.parse::<i64>()
        .map_err(|_| format!("invalid immediate: {}", op))?;

    let value = sign * parsed;

    if sign < 0 {
        Ok(Operand::ImmI32(value as i32))
    } else {
        Ok(Operand::ImmU32(value as u32))
    }
}

fn parse_number_u32(s: &str) -> Result<u32, String> {
    if let Some(hex) = s.strip_prefix("0x") {
        u32::from_str_radix(hex, 16)
            .map_err(|_| format!("invalid hex number: {}", s))
    } else if let Some(bin) = s.strip_prefix("0b") {
        u32::from_str_radix(bin, 2)
            .map_err(|_| format!("invalid binary number: {}", s))
    } else {
        s.parse::<u32>()
            .map_err(|_| format!("invalid number: {}", s))
    }
}

// s_waitcnt special parsing
fn parse_waitcnt(operands_str: &str) -> Result<ParsedInstruction, String> {
    let operands_str = operands_str.trim();

    // Check if it's just a raw immediate
    if !operands_str.contains('(') {
        let imm = parse_number_u32(operands_str)?;
        return Ok(ParsedInstruction {
            name: "s_waitcnt".to_string(),
            operands: vec![Operand::ImmU32(imm)],
        });
    }

    // Parse structured form: vmcnt(N) lgkmcnt(N) expcnt(N)
    let mut vmcnt = 63u32; // default
    let mut lgkmcnt = 63u32; // default
    let mut expcnt = 7u32; // default

    // Split by whitespace, comma, or ampersand
    let parts: Vec<&str> = operands_str
        .split(|c: char| c.is_whitespace() || c == ',' || c == '&')
        .filter(|s| !s.is_empty())
        .collect();

    for part in parts {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }

        if let Some(inner) = extract_function_arg(part, "vmcnt") {
            vmcnt = parse_counter_value(inner, 63)?;
        } else if let Some(inner) = extract_function_arg(part, "vmcnt_sat") {
            vmcnt = parse_counter_value_any(inner)?.min(63);
        } else if let Some(inner) = extract_function_arg(part, "lgkmcnt") {
            lgkmcnt = parse_counter_value(inner, 63)?;
        } else if let Some(inner) = extract_function_arg(part, "lgkmcnt_sat") {
            lgkmcnt = parse_counter_value_any(inner)?.min(63);
        } else if let Some(inner) = extract_function_arg(part, "expcnt") {
            expcnt = parse_counter_value(inner, 7)?;
        } else if let Some(inner) = extract_function_arg(part, "expcnt_sat") {
            expcnt = parse_counter_value_any(inner)?.min(7);
        } else {
            return Err(format!("invalid s_waitcnt token: {}", part));
        }
    }

    // Pack into 16-bit immediate: imm16 = (vmcnt << 10) | (lgkmcnt << 4) | expcnt
    let imm16 = (vmcnt << 10) | (lgkmcnt << 4) | expcnt;

    Ok(ParsedInstruction {
        name: "s_waitcnt".to_string(),
        operands: vec![Operand::ImmU32(imm16)],
    })
}

fn extract_function_arg<'a>(s: &'a str, func_name: &str) -> Option<&'a str> {
    let s = s.trim();
    if let Some(rest) = s.strip_prefix(func_name) {
        if let Some(inner) = rest.strip_prefix('(') {
            if let Some(end_pos) = inner.find(')') {
                return Some(inner[..end_pos].trim());
            }
        }
    }
    None
}

fn parse_counter_value(s: &str, max: u32) -> Result<u32, String> {
    let value = parse_number_u32(s)?;
    if value > max {
        return Err(format!("s_waitcnt value out of range (max {}): {}", max, s));
    }
    Ok(value)
}

fn parse_counter_value_any(s: &str) -> Result<u32, String> {
    parse_number_u32(s)
}

// s_sendmsg special parsing
fn parse_sendmsg(operands_str: &str) -> Result<ParsedInstruction, String> {
    let operands_str = operands_str.trim();

    // Check if it's just a raw immediate
    if !operands_str.contains("sendmsg") {
        let imm = parse_number_u32(operands_str)?;
        return Ok(ParsedInstruction {
            name: "s_sendmsg".to_string(),
            operands: vec![Operand::ImmU32(imm)],
        });
    }

    // Parse sendmsg(...) form
    if let Some(inner) = extract_function_arg(operands_str, "sendmsg") {
        let parts: Vec<&str> = inner.split(',').map(|s| s.trim()).collect();

        let msg_type = parse_sendmsg_type(parts[0])?;
        let op = if parts.len() > 1 {
            parse_number_u32(parts[1])?
        } else {
            0
        };
        let stream = if parts.len() > 2 {
            parse_number_u32(parts[2])?
        } else {
            0
        };

        // Pack: imm16 = (stream << 8) | (op << 4) | type
        let imm16 = (stream << 8) | (op << 4) | msg_type;

        return Ok(ParsedInstruction {
            name: "s_sendmsg".to_string(),
            operands: vec![Operand::ImmU32(imm16)],
        });
    }

    Err("invalid s_sendmsg format".to_string())
}

fn parse_sendmsg_type(s: &str) -> Result<u32, String> {
    let lower = s.to_ascii_lowercase();
    match lower.as_str() {
        "msg_interrupt" => Ok(1),
        "msg_dealloc_vgprs" => Ok(3),
        _ => parse_number_u32(s),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Helper functions for concise test writing
    fn sgpr(idx: u16) -> Operand {
        Operand::Sgpr(idx)
    }

    fn vgpr(idx: u16) -> Operand {
        Operand::Vgpr(idx)
    }

    fn sgpr_range(start: u16, end: u16) -> Operand {
        Operand::SgprRange(start, end)
    }

    fn vgpr_range(start: u16, end: u16) -> Operand {
        Operand::VgprRange(start, end)
    }

    fn imm_u32(val: u32) -> Operand {
        Operand::ImmU32(val)
    }

    fn imm_i32(val: i32) -> Operand {
        Operand::ImmI32(val)
    }

    fn imm_f32(val: f32) -> Operand {
        Operand::ImmF32(val)
    }

    fn special(reg: SpecialRegister) -> Operand {
        Operand::SpecialReg(reg)
    }

    fn negate(op: Operand) -> Operand {
        Operand::Negate(Box::new(op))
    }

    fn abs(op: Operand) -> Operand {
        Operand::Abs(Box::new(op))
    }

    fn offset(val: u32) -> Operand {
        Operand::Offset(val)
    }

    fn flag(name: &str) -> Operand {
        Operand::Flag(name.to_string())
    }

    // Basic instruction parsing tests
    #[test]
    fn test_simple_instruction_no_operands() {
        let inst = parse_instruction("s_endpgm").unwrap();
        assert_eq!(inst.name, "s_endpgm");
        assert_eq!(inst.operands.len(), 0);
    }

    #[test]
    fn test_instruction_with_encoding_suffix() {
        // Encodings like _e64, _e32 should be stripped
        let inst = parse_instruction("v_add_f32_e32 v0, v1, v2").unwrap();
        assert_eq!(inst.name, "v_add_f32");
        assert_eq!(inst.operands, vec![vgpr(0), vgpr(1), vgpr(2)]);

        let inst = parse_instruction("v_cmp_eq_i32_e64 s[0:1], v0, v1").unwrap();
        assert_eq!(inst.name, "v_cmp_eq_i32");
        assert_eq!(inst.operands, vec![sgpr_range(0, 1), vgpr(0), vgpr(1)]);
    }

    #[test]
    fn test_instruction_case_insensitive() {
        let inst1 = parse_instruction("S_MOV_B32 s0, 0").unwrap();
        let inst2 = parse_instruction("s_mov_b32 s0, 0").unwrap();
        assert_eq!(inst1.name, inst2.name);
    }

    // Register operand tests
    #[test]
    fn test_scalar_register() {
        let inst = parse_instruction("s_mov_b32 s0, s1").unwrap();
        assert_eq!(inst.operands, vec![sgpr(0), sgpr(1)]);

        let inst = parse_instruction("s_mov_b32 s42, s99").unwrap();
        assert_eq!(inst.operands, vec![sgpr(42), sgpr(99)]);
    }

    #[test]
    fn test_vector_register() {
        let inst = parse_instruction("v_mov_b32 v0, v1").unwrap();
        assert_eq!(inst.operands, vec![vgpr(0), vgpr(1)]);

        let inst = parse_instruction("v_add_f32 v10, v20, v30").unwrap();
        assert_eq!(inst.operands, vec![vgpr(10), vgpr(20), vgpr(30)]);
    }

    #[test]
    fn test_scalar_register_range() {
        let inst = parse_instruction("s_load_b64 s[0:1], s[2:3], 0").unwrap();
        assert_eq!(inst.operands, vec![
            sgpr_range(0, 1),
            sgpr_range(2, 3),
            imm_u32(0)
        ]);

        let inst = parse_instruction("s_load_b128 s[4:7], s[8:9], 0").unwrap();
        assert_eq!(inst.operands, vec![
            sgpr_range(4, 7),
            sgpr_range(8, 9),
            imm_u32(0)
        ]);
    }

    #[test]
    fn test_vector_register_range() {
        let inst = parse_instruction("buffer_load_b128 v[0:3], v4, s[0:3], 0").unwrap();
        assert_eq!(inst.operands, vec![
            vgpr_range(0, 3),
            vgpr(4),
            sgpr_range(0, 3),
            imm_u32(0)
        ]);
    }

    #[test]
    fn test_mixed_registers() {
        let inst = parse_instruction("v_add_co_u32 v0, vcc, v1, v2").unwrap();
        assert_eq!(inst.operands, vec![
            vgpr(0),
            special(SpecialRegister::Vcc),
            vgpr(1),
            vgpr(2)
        ]);
    }

    // Special register tests
    #[test]
    fn test_special_registers() {
        let inst = parse_instruction("s_mov_b64 exec, s[0:1]").unwrap();
        assert_eq!(inst.operands, vec![
            special(SpecialRegister::Exec),
            sgpr_range(0, 1)
        ]);

        let inst = parse_instruction("s_mov_b32 exec_lo, s0").unwrap();
        assert_eq!(inst.operands, vec![
            special(SpecialRegister::ExecLo),
            sgpr(0)
        ]);

        let inst = parse_instruction("s_mov_b32 exec_hi, s1").unwrap();
        assert_eq!(inst.operands, vec![
            special(SpecialRegister::ExecHi),
            sgpr(1)
        ]);

        let inst = parse_instruction("v_cmp_eq_i32 vcc, v0, v1").unwrap();
        assert_eq!(inst.operands, vec![
            special(SpecialRegister::Vcc),
            vgpr(0),
            vgpr(1)
        ]);

        let inst = parse_instruction("s_mov_b32 m0, s0").unwrap();
        assert_eq!(inst.operands, vec![
            special(SpecialRegister::M0),
            sgpr(0)
        ]);

        let inst = parse_instruction("s_mov_b32 null, s0").unwrap();
        assert_eq!(inst.operands, vec![
            special(SpecialRegister::Null),
            sgpr(0)
        ]);

        let inst = parse_instruction("s_cselect_b32 s0, s1, s2, scc").unwrap();
        assert_eq!(inst.operands, vec![
            sgpr(0),
            sgpr(1),
            sgpr(2),
            special(SpecialRegister::Scc)
        ]);
    }

    #[test]
    fn test_vcc_lo_hi() {
        let inst = parse_instruction("s_mov_b32 vcc_lo, s0").unwrap();
        assert_eq!(inst.operands, vec![
            special(SpecialRegister::VccLo),
            sgpr(0)
        ]);

        let inst = parse_instruction("s_mov_b32 vcc_hi, s1").unwrap();
        assert_eq!(inst.operands, vec![
            special(SpecialRegister::VccHi),
            sgpr(1)
        ]);
    }

    // Operand modifier tests
    #[test]
    fn test_negate_modifier() {
        let inst = parse_instruction("v_add_f32 v0, -v1, v2").unwrap();
        assert_eq!(inst.operands, vec![
            vgpr(0),
            negate(vgpr(1)),
            vgpr(2)
        ]);

        let inst = parse_instruction("v_mul_f32 v0, v1, -v2").unwrap();
        assert_eq!(inst.operands, vec![
            vgpr(0),
            vgpr(1),
            negate(vgpr(2))
        ]);
    }

    #[test]
    fn test_abs_modifier() {
        let inst = parse_instruction("v_add_f32 v0, |v1|, v2").unwrap();
        assert_eq!(inst.operands, vec![
            vgpr(0),
            abs(vgpr(1)),
            vgpr(2)
        ]);

        let inst = parse_instruction("v_mul_f32 v0, |v1|, |v2|").unwrap();
        assert_eq!(inst.operands, vec![
            vgpr(0),
            abs(vgpr(1)),
            abs(vgpr(2))
        ]);
    }

    #[test]
    fn test_negate_abs_combined() {
        // -|v1| should parse as negate(abs(v1))
        let inst = parse_instruction("v_add_f32 v0, -|v1|, v2").unwrap();
        assert_eq!(inst.operands, vec![
            vgpr(0),
            negate(abs(vgpr(1))),
            vgpr(2)
        ]);
    }

    // Immediate value tests
    #[test]
    fn test_decimal_immediate() {
        let inst = parse_instruction("s_mov_b32 s0, 0").unwrap();
        assert_eq!(inst.operands, vec![sgpr(0), imm_u32(0)]);

        let inst = parse_instruction("s_mov_b32 s0, 42").unwrap();
        assert_eq!(inst.operands, vec![sgpr(0), imm_u32(42)]);

        let inst = parse_instruction("s_mov_b32 s0, 1234567").unwrap();
        assert_eq!(inst.operands, vec![sgpr(0), imm_u32(1234567)]);
    }

    #[test]
    fn test_negative_immediate() {
        let inst = parse_instruction("s_mov_b32 s0, -1").unwrap();
        assert_eq!(inst.operands, vec![sgpr(0), imm_i32(-1)]);

        let inst = parse_instruction("s_mov_b32 s0, -42").unwrap();
        assert_eq!(inst.operands, vec![sgpr(0), imm_i32(-42)]);

        let inst = parse_instruction("v_add_i32 v0, v1, -10").unwrap();
        assert_eq!(inst.operands, vec![
            vgpr(0),
            vgpr(1),
            imm_i32(-10)
        ]);
    }

    #[test]
    fn test_hex_immediate() {
        let inst = parse_instruction("s_mov_b32 s0, 0x0").unwrap();
        assert_eq!(inst.operands, vec![sgpr(0), imm_u32(0)]);

        let inst = parse_instruction("s_mov_b32 s0, 0x2a").unwrap();
        assert_eq!(inst.operands, vec![sgpr(0), imm_u32(42)]);

        let inst = parse_instruction("s_mov_b32 s0, 0xFF").unwrap();
        assert_eq!(inst.operands, vec![sgpr(0), imm_u32(255)]);

        let inst = parse_instruction("s_mov_b32 s0, 0xDEADBEEF").unwrap();
        assert_eq!(inst.operands, vec![sgpr(0), imm_u32(0xDEADBEEF)]);
    }

    #[test]
    fn test_binary_immediate() {
        let inst = parse_instruction("s_mov_b32 s0, 0b0").unwrap();
        assert_eq!(inst.operands, vec![sgpr(0), imm_u32(0)]);

        let inst = parse_instruction("s_mov_b32 s0, 0b1010").unwrap();
        assert_eq!(inst.operands, vec![sgpr(0), imm_u32(10)]);

        let inst = parse_instruction("s_mov_b32 s0, 0b11111111").unwrap();
        assert_eq!(inst.operands, vec![sgpr(0), imm_u32(255)]);
    }

    #[test]
    fn test_float_immediate() {
        let inst = parse_instruction("v_mov_b32 v0, 1.0").unwrap();
        assert_eq!(inst.operands, vec![vgpr(0), imm_f32(1.0)]);

        let inst = parse_instruction("v_mov_b32 v0, -2.5").unwrap();
        assert_eq!(inst.operands, vec![vgpr(0), imm_f32(-2.5)]);

        let inst = parse_instruction("v_mul_f32 v0, v1, 0.5").unwrap();
        assert_eq!(inst.operands, vec![
            vgpr(0),
            vgpr(1),
            imm_f32(0.5)
        ]);

        let inst = parse_instruction("v_add_f32 v0, v1, 3.14159").unwrap();
        assert_eq!(inst.operands, vec![
            vgpr(0),
            vgpr(1),
            imm_f32(3.14159)
        ]);
    }

    #[test]
    fn test_scientific_notation_float() {
        let inst = parse_instruction("v_mov_b32 v0, 1.5e2").unwrap();
        assert_eq!(inst.operands, vec![vgpr(0), imm_f32(150.0)]);

        let inst = parse_instruction("v_mov_b32 v0, 2.5E-1").unwrap();
        assert_eq!(inst.operands, vec![vgpr(0), imm_f32(0.25)]);
    }

    // Offset tests
    #[test]
    fn test_offset() {
        let inst = parse_instruction("buffer_load_b32 v0, v1, s[0:3], 0 offset:256").unwrap();
        assert_eq!(inst.operands, vec![
            vgpr(0),
            vgpr(1),
            sgpr_range(0, 3),
            imm_u32(0),
            offset(256)
        ]);

        let inst = parse_instruction("ds_read_b32 v0, v1 offset:1024").unwrap();
        assert_eq!(inst.operands, vec![
            vgpr(0),
            vgpr(1),
            offset(1024)
        ]);
    }

    // Cache policy and flag tests
    #[test]
    fn test_cache_policy_flags() {
        let inst = parse_instruction("buffer_load_b32 v0, v1, s[0:3], 0 glc").unwrap();
        assert_eq!(inst.operands, vec![
            vgpr(0),
            vgpr(1),
            sgpr_range(0, 3),
            imm_u32(0),
            flag("glc")
        ]);

        let inst = parse_instruction("buffer_store_b32 v0, v1, s[0:3], 0 glc slc").unwrap();
        assert_eq!(inst.operands, vec![
            vgpr(0),
            vgpr(1),
            sgpr_range(0, 3),
            imm_u32(0),
            flag("glc"),
            flag("slc")
        ]);

        let inst = parse_instruction("buffer_load_b32 v0, v1, s[0:3], 0 glc slc nt").unwrap();
        assert_eq!(inst.operands, vec![
            vgpr(0),
            vgpr(1),
            sgpr_range(0, 3),
            imm_u32(0),
            flag("glc"),
            flag("slc"),
            flag("nt")
        ]);
    }

    #[test]
    fn test_offen_idxen_flags() {
        let inst = parse_instruction("buffer_load_b32 v0, v1, s[0:3], 0 offen").unwrap();
        assert_eq!(inst.operands, vec![
            vgpr(0),
            vgpr(1),
            sgpr_range(0, 3),
            imm_u32(0),
            flag("offen")
        ]);

        let inst = parse_instruction("buffer_load_b32 v0, v1, s[0:3], 0 idxen").unwrap();
        assert_eq!(inst.operands, vec![
            vgpr(0),
            vgpr(1),
            sgpr_range(0, 3),
            imm_u32(0),
            flag("idxen")
        ]);

        let inst = parse_instruction("buffer_load_b32 v0, v1, s[0:3], 0 offen idxen").unwrap();
        assert_eq!(inst.operands, vec![
            vgpr(0),
            vgpr(1),
            sgpr_range(0, 3),
            imm_u32(0),
            flag("offen"),
            flag("idxen")
        ]);
    }

    // s_waitcnt special parsing tests
    #[test]
    fn test_waitcnt_vmcnt_only() {
        let inst = parse_instruction("s_waitcnt vmcnt(0)").unwrap();
        assert_eq!(inst.name, "s_waitcnt");
        // vmcnt=0, lgkmcnt=63 (default), expcnt=7 (default)
        // imm16 = (0 << 10) | (63 << 4) | 7 = 0x03F7
        assert_eq!(inst.operands, vec![imm_u32(0x03F7)]);
    }

    #[test]
    fn test_waitcnt_lgkmcnt_only() {
        let inst = parse_instruction("s_waitcnt lgkmcnt(0)").unwrap();
        assert_eq!(inst.name, "s_waitcnt");
        // vmcnt=63 (default), lgkmcnt=0, expcnt=7 (default)
        // imm16 = (63 << 10) | (0 << 4) | 7 = 0xFC07
        assert_eq!(inst.operands, vec![imm_u32(0xFC07)]);
    }

    #[test]
    fn test_waitcnt_expcnt_only() {
        let inst = parse_instruction("s_waitcnt expcnt(0)").unwrap();
        assert_eq!(inst.name, "s_waitcnt");
        // vmcnt=63 (default), lgkmcnt=63 (default), expcnt=0
        // imm16 = (63 << 10) | (63 << 4) | 0 = 0xFFF0
        assert_eq!(inst.operands, vec![imm_u32(0xFFF0)]);
    }

    #[test]
    fn test_waitcnt_vmcnt_lgkmcnt() {
        let inst = parse_instruction("s_waitcnt vmcnt(0) lgkmcnt(0)").unwrap();
        assert_eq!(inst.name, "s_waitcnt");
        // vmcnt=0, lgkmcnt=0, expcnt=7 (default)
        // imm16 = (0 << 10) | (0 << 4) | 7 = 0x0007
        assert_eq!(inst.operands, vec![imm_u32(0x0007)]);
    }

    #[test]
    fn test_waitcnt_all_fields() {
        let inst = parse_instruction("s_waitcnt vmcnt(2) lgkmcnt(3) expcnt(1)").unwrap();
        assert_eq!(inst.name, "s_waitcnt");
        // vmcnt=2, lgkmcnt=3, expcnt=1
        // imm16 = (2 << 10) | (3 << 4) | 1 = 0x0831
        assert_eq!(inst.operands, vec![imm_u32(0x0831)]);
    }

    #[test]
    fn test_waitcnt_order_independent() {
        let inst1 = parse_instruction("s_waitcnt lgkmcnt(0) vmcnt(0)").unwrap();
        let inst2 = parse_instruction("s_waitcnt vmcnt(0) lgkmcnt(0)").unwrap();
        assert_eq!(inst1.operands, inst2.operands);
    }

    #[test]
    fn test_waitcnt_immediate_form() {
        let inst = parse_instruction("s_waitcnt 0").unwrap();
        assert_eq!(inst.name, "s_waitcnt");
        assert_eq!(inst.operands, vec![imm_u32(0)]);
    }

    #[test]
    fn test_waitcnt_immediate_hex_form() {
        let inst = parse_instruction("s_waitcnt 0xFC07").unwrap();
        assert_eq!(inst.name, "s_waitcnt");
        assert_eq!(inst.operands, vec![imm_u32(0xFC07)]);
    }

    #[test]
    fn test_waitcnt_with_ampersand_separator() {
        // Some assemblers allow & as separator
        let inst = parse_instruction("s_waitcnt vmcnt(0) & lgkmcnt(0)").unwrap();
        assert_eq!(inst.name, "s_waitcnt");
        assert_eq!(inst.operands, vec![imm_u32(0x0007)]);
    }

    #[test]
    fn test_waitcnt_max_values() {
        let inst = parse_instruction("s_waitcnt vmcnt(63) lgkmcnt(63) expcnt(7)").unwrap();
        assert_eq!(inst.name, "s_waitcnt");
        // vmcnt=63, lgkmcnt=63, expcnt=7
        // imm16 = (63 << 10) | (63 << 4) | 7 = 0xFFF7
        assert_eq!(inst.operands, vec![imm_u32(0xFFF7)]);
    }

    #[test]
    fn test_waitcnt_invalid_value_rejected() {
        let err = parse_instruction("s_waitcnt lgkmcnt(64) vmcnt(0) expcnt(0)")
            .unwrap_err();
        assert!(err.contains("out of range"));
    }

    #[test]
    fn test_waitcnt_invalid_token_rejected() {
        let err = parse_instruction("s_waitcnt lgkmct(0) vmcnt(0) expcnt(0)")
            .unwrap_err();
        assert!(err.contains("invalid s_waitcnt token"));
    }

    // s_sendmsg special parsing tests
    #[test]
    fn test_sendmsg_interrupt() {
        let inst = parse_instruction("s_sendmsg sendmsg(MSG_INTERRUPT)").unwrap();
        assert_eq!(inst.name, "s_sendmsg");
        // MSG_INTERRUPT = 1, op=0, stream=0
        // imm16 = (0 << 8) | (0 << 4) | 1 = 0x0001
        assert_eq!(inst.operands, vec![imm_u32(0x0001)]);
    }

    #[test]
    fn test_sendmsg_dealloc_vgprs() {
        let inst = parse_instruction("s_sendmsg sendmsg(MSG_DEALLOC_VGPRS)").unwrap();
        assert_eq!(inst.name, "s_sendmsg");
        // MSG_DEALLOC_VGPRS = 3, op=0, stream=0
        // imm16 = (0 << 8) | (0 << 4) | 3 = 0x0003
        assert_eq!(inst.operands, vec![imm_u32(0x0003)]);
    }

    #[test]
    fn test_sendmsg_numeric_form() {
        let inst = parse_instruction("s_sendmsg sendmsg(1)").unwrap();
        assert_eq!(inst.name, "s_sendmsg");
        assert_eq!(inst.operands, vec![imm_u32(0x0001)]);

        let inst = parse_instruction("s_sendmsg sendmsg(3)").unwrap();
        assert_eq!(inst.name, "s_sendmsg");
        assert_eq!(inst.operands, vec![imm_u32(0x0003)]);
    }

    #[test]
    fn test_sendmsg_with_op_stream() {
        let inst = parse_instruction("s_sendmsg sendmsg(1, 2, 1)").unwrap();
        assert_eq!(inst.name, "s_sendmsg");
        // type=1, op=2, stream=1
        // imm16 = (1 << 8) | (2 << 4) | 1 = 0x0121
        assert_eq!(inst.operands, vec![imm_u32(0x0121)]);
    }

    #[test]
    fn test_sendmsg_immediate_form() {
        let inst = parse_instruction("s_sendmsg 0x0001").unwrap();
        assert_eq!(inst.name, "s_sendmsg");
        assert_eq!(inst.operands, vec![imm_u32(0x0001)]);
    }

    // Complex instruction tests
    #[test]
    fn test_complex_instruction_with_multiple_operands() {
        let inst = parse_instruction(
            "buffer_load_b128 v[0:3], v4, s[8:11], s12 offen offset:64 glc"
        ).unwrap();
        assert_eq!(inst.name, "buffer_load_b128");
        assert_eq!(inst.operands, vec![
            vgpr_range(0, 3),
            vgpr(4),
            sgpr_range(8, 11),
            sgpr(12),
            flag("offen"),
            offset(64),
            flag("glc")
        ]);
    }

    #[test]
    fn test_real_world_load_store() {
        let inst = parse_instruction("global_load_b32 v0, v[1:2], off").unwrap();
        assert_eq!(inst.name, "global_load_b32");
        assert_eq!(inst.operands, vec![
            vgpr(0),
            vgpr_range(1, 2),
            flag("off")
        ]);
    }

    // Edge case tests
    #[test]
    fn test_whitespace_handling() {
        let inst1 = parse_instruction("s_mov_b32 s0, 0").unwrap();
        let inst2 = parse_instruction("s_mov_b32  s0,  0").unwrap();
        let inst3 = parse_instruction("s_mov_b32   s0,   0").unwrap();
        assert_eq!(inst1.operands, inst2.operands);
        assert_eq!(inst1.operands, inst3.operands);
    }

    #[test]
    fn test_trailing_whitespace() {
        let inst = parse_instruction("s_mov_b32 s0, 0   ").unwrap();
        assert_eq!(inst.name, "s_mov_b32");
        assert_eq!(inst.operands, vec![sgpr(0), imm_u32(0)]);
    }

    #[test]
    fn test_empty_instruction() {
        let result = parse_instruction("");
        assert!(result.is_err());
    }

    #[test]
    fn test_whitespace_only() {
        let result = parse_instruction("   ");
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_register_format() {
        let result = parse_instruction("s_mov_b32 s, 0");
        assert!(result.is_err());

        let result = parse_instruction("s_mov_b32 s[], 0");
        assert!(result.is_err());

        let result = parse_instruction("s_mov_b32 s[1:], 0");
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_range_order() {
        // End should be >= start
        let result = parse_instruction("s_load_b64 s[3:1], s[0:1], 0");
        assert!(result.is_err());
    }

    #[test]
    fn test_negate_immediate() {
        // Negating an immediate should result in a negative immediate
        let inst = parse_instruction("v_add_f32 v0, v1, -1.5").unwrap();
        assert_eq!(inst.operands, vec![
            vgpr(0),
            vgpr(1),
            imm_f32(-1.5)
        ]);
    }

    #[test]
    fn test_instruction_with_no_comma_separation() {
        // Some instructions might have space-separated operands
        let inst = parse_instruction("s_nop 0").unwrap();
        assert_eq!(inst.name, "s_nop");
        assert_eq!(inst.operands, vec![imm_u32(0)]);
    }

    #[test]
    fn test_register_bracket_variations() {
        // Test with and without spaces in brackets
        let inst1 = parse_instruction("s_load_b64 s[0:1], s[2:3], 0").unwrap();
        let inst2 = parse_instruction("s_load_b64 s[ 0:1 ], s[ 2:3 ], 0").unwrap();
        assert_eq!(inst1.operands, inst2.operands);
    }

    #[test]
    fn test_offset_with_hex() {
        let inst = parse_instruction("ds_read_b32 v0, v1 offset:0x400").unwrap();
        assert_eq!(inst.operands, vec![
            vgpr(0),
            vgpr(1),
            offset(0x400)
        ]);
    }

    #[test]
    fn test_multiple_encodings_stripped() {
        let inst = parse_instruction("v_add_f32_e64 v0, v1, v2").unwrap();
        assert_eq!(inst.name, "v_add_f32");

        let inst = parse_instruction("v_cmp_eq_i32_e32 vcc, v0, v1").unwrap();
        assert_eq!(inst.name, "v_cmp_eq_i32");
    }

    #[test]
    fn test_case_preservation_in_flags() {
        // Flags should preserve case (or be lowercased)
        let inst = parse_instruction("buffer_load_b32 v0, v1, s[0:3], 0 GLC").unwrap();
        // We'll normalize to lowercase in implementation
        assert_eq!(inst.operands[4], flag("glc"));
    }

    #[test]
    fn test_large_register_numbers() {
        let inst = parse_instruction("s_mov_b32 s255, s254").unwrap();
        assert_eq!(inst.operands, vec![sgpr(255), sgpr(254)]);

        let inst = parse_instruction("v_mov_b32 v255, v254").unwrap();
        assert_eq!(inst.operands, vec![vgpr(255), vgpr(254)]);
    }

    #[test]
    fn test_large_register_ranges() {
        let inst = parse_instruction("s_load_b512 s[0:15], s[16:17], 0").unwrap();
        assert_eq!(inst.operands, vec![
            sgpr_range(0, 15),
            sgpr_range(16, 17),
            imm_u32(0)
        ]);
    }

    #[test]
    fn test_waitcnt_vmcnt_sat() {
        // vmcnt_sat should clamp to max value (63)
        let inst = parse_instruction("s_waitcnt vmcnt_sat(100)").unwrap();
        // Should clamp 100 to 63
        // vmcnt=63, lgkmcnt=63 (default), expcnt=7 (default)
        assert_eq!(inst.operands, vec![imm_u32(0xFFF7)]);
    }

    #[test]
    fn test_sendmsg_case_insensitive() {
        let inst1 = parse_instruction("s_sendmsg sendmsg(MSG_INTERRUPT)").unwrap();
        let inst2 = parse_instruction("s_sendmsg sendmsg(msg_interrupt)").unwrap();
        assert_eq!(inst1.operands, inst2.operands);
    }

    #[test]
    fn test_mixed_operand_types() {
        let inst = parse_instruction("v_mad_f32 v0, v1, -2.0, |v2|").unwrap();
        assert_eq!(inst.operands, vec![
            vgpr(0),
            vgpr(1),
            imm_f32(-2.0),
            abs(vgpr(2))
        ]);
    }

    #[test]
    fn test_reject_abs_on_vgpr_range() {
        // Absolute value on register range should be rejected
        let result = parse_instruction("v_add_f32 v0, |v[1:2]|, v3");
        assert!(result.is_err(), "Expected error for absolute value on VGPR range");
        let err = result.unwrap_err();
        assert!(err.contains("cannot apply absolute value modifier to register range"),
                "Error message should mention absolute value on range: {}", err);
    }

    #[test]
    fn test_special_register_case_insensitive() {
        let inst1 = parse_instruction("s_mov_b64 EXEC, s[0:1]").unwrap();
        let inst2 = parse_instruction("s_mov_b64 exec, s[0:1]").unwrap();
        assert_eq!(inst1.operands, inst2.operands);
    }

    // Modifier validation tests - reject modifiers on register ranges
    #[test]
    fn test_reject_negate_on_vgpr_range() {
        // Negation on VGPR range should be rejected
        let result = parse_instruction("v_add_f32 v0, -v[0:3], v1");
        assert!(result.is_err(), "Expected error for negation on VGPR range");
        let err = result.unwrap_err();
        assert!(err.contains("cannot apply negation modifier to register range"),
                "Error message should mention negation on range: {}", err);
    }

    #[test]
    fn test_reject_abs_on_sgpr_range() {
        // Absolute value on SGPR range should be rejected
        let result = parse_instruction("v_mul_f32 v0, |s[2:5]|, v1");
        assert!(result.is_err(), "Expected error for absolute value on SGPR range");
        let err = result.unwrap_err();
        assert!(err.contains("cannot apply absolute value modifier to register range"),
                "Error message should mention absolute value on range: {}", err);
    }

    #[test]
    fn test_reject_combined_modifiers_on_range() {
        // Combined modifiers on register range should be rejected
        let result = parse_instruction("v_mad_f32 v0, -|v[1:2]|, v3, v4");
        assert!(result.is_err(), "Expected error for combined modifiers on VGPR range");
        let err = result.unwrap_err();
        // Will fail on the abs modifier check
        assert!(err.contains("cannot apply absolute value modifier to register range"),
                "Error message should mention modifier on range: {}", err);
    }

    #[test]
    fn test_reject_negate_on_sgpr_range() {
        // Negation on SGPR range should be rejected
        let result = parse_instruction("v_add_f32 v0, -s[1:4], v1");
        assert!(result.is_err(), "Expected error for negation on SGPR range");
        let err = result.unwrap_err();
        assert!(err.contains("cannot apply negation modifier to register range"),
                "Error message should mention negation on range: {}", err);
    }
}
