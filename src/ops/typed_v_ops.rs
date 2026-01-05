use crate::isa::types::DataType;
use crate::sim::{Ctx, DecodedOperand, ExecError, ExecResult, NumericType};
use half::{bf16, f16};

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum BinaryOp {
  Add,
  Sub,
  Mul,
  Div,
  Min,
  Max,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum CmpOp {
  Eq,
  Ne,
  Lt,
  Le,
  Gt,
  Ge,
  Lg,
  Neq,
  Nlt,
  Nle,
  Ngt,
  Nge,
  Nlg,
  O,
  U,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum TypedVHandler {
  Binary { dtype: DataType, op: BinaryOp },
  Cmp { dtype: DataType, op: CmpOp, update_exec: bool },
  Unknown { dtype: DataType },
}

pub fn run_typed_v(ctx: &mut Ctx, name: &'static str, handler: TypedVHandler) -> ExecResult {
  match handler {
    TypedVHandler::Binary { dtype, op } => binary_op_dispatch(ctx, dtype, op),
    TypedVHandler::Cmp { dtype, op, update_exec } => cmp_op_dispatch(ctx, dtype, op, update_exec),
    TypedVHandler::Unknown { .. } => Err(ExecError::Unimplemented(name)),
  }
}

fn unpack_vgpr_modifiers(operand: &DecodedOperand) -> (u16, bool, bool) {
  let mut abs = false;
  let mut neg = false;
  let mut current = operand;

  loop {
    match current {
      DecodedOperand::Vgpr(idx) => return (*idx, abs, neg),
      DecodedOperand::Abs(inner) => {
        abs = true;
        current = inner;
      }
      DecodedOperand::Negate(inner) => {
        neg = true;
        current = inner;
      }
      _ => panic!("expected Vgpr operand"),
    }
  }
}

fn read_vgpr_f16(ctx: &Ctx, operand: &DecodedOperand, lane: usize) -> f16 {
  let (idx, abs, neg) = unpack_vgpr_modifiers(operand);
  let bits = ctx.wave.read_vgpr(idx as usize, lane) as u16;
  let mut value = f16::from_bits(bits).to_f32();
  if abs {
    value = value.abs();
  }
  if neg {
    value = -value;
  }
  f16::from_f32(value)
}

fn read_vgpr_bf16(ctx: &Ctx, operand: &DecodedOperand, lane: usize) -> bf16 {
  let (idx, abs, neg) = unpack_vgpr_modifiers(operand);
  let bits = ctx.wave.read_vgpr(idx as usize, lane) as u16;
  let mut value = bf16::from_bits(bits).to_f32();
  if abs {
    value = value.abs();
  }
  if neg {
    value = -value;
  }
  bf16::from_f32(value)
}

fn read_operand_f32(ctx: &Ctx, operand: &DecodedOperand, lane: usize) -> f32 {
  match operand {
    DecodedOperand::ImmF32(v) => *v,
    DecodedOperand::ImmI32(v) => *v as f32,
    DecodedOperand::ImmU32(v) => *v as f32,
    _ => ctx.read_vgpr::<f32>(operand, lane),
  }
}

fn read_operand_f16(ctx: &Ctx, operand: &DecodedOperand, lane: usize) -> f16 {
  match operand {
    DecodedOperand::ImmF32(v) => f16::from_f32(*v),
    DecodedOperand::ImmI32(v) => f16::from_f32(*v as f32),
    DecodedOperand::ImmU32(v) => f16::from_f32(*v as f32),
    _ => read_vgpr_f16(ctx, operand, lane),
  }
}

fn read_operand_bf16(ctx: &Ctx, operand: &DecodedOperand, lane: usize) -> bf16 {
  match operand {
    DecodedOperand::ImmF32(v) => bf16::from_f32(*v),
    DecodedOperand::ImmI32(v) => bf16::from_f32(*v as f32),
    DecodedOperand::ImmU32(v) => bf16::from_f32(*v as f32),
    _ => read_vgpr_bf16(ctx, operand, lane),
  }
}

fn read_operand_int<T: NumericType>(ctx: &Ctx, operand: &DecodedOperand, lane: usize) -> T {
  match operand {
    DecodedOperand::Vgpr(n) => T::from_bits(ctx.wave.read_vgpr(*n as usize, lane)),
    DecodedOperand::ImmU32(v) => T::from_bits(*v),
    DecodedOperand::ImmI32(v) => T::from_bits(*v as u32),
    DecodedOperand::ImmF32(v) => T::from_bits(*v as i32 as u32),
    DecodedOperand::Abs(_) | DecodedOperand::Negate(_) => {
      panic!("unexpected modifiers for integer operand")
    }
    _ => panic!("expected vgpr or imm"),
  }
}

fn binary_op_with<T: NumericType, F: Fn(&Ctx, &DecodedOperand, usize) -> T>(
  ctx: &mut Ctx,
  op: BinaryOp,
  read_operand: F,
) -> ExecResult {
  let dst = ctx.dst_vgpr();
  let src1 = ctx.inst.operands[1].clone();
  let src2 = ctx.inst.operands[2].clone();

  for lane in 0..ctx.wave.wave_lanes() {
    let a = read_operand(ctx, &src1, lane);
    let b = read_operand(ctx, &src2, lane);
    let result = match op {
      BinaryOp::Add => a.add(b),
      BinaryOp::Sub => a.sub(b),
      BinaryOp::Mul => a.mul(b),
      BinaryOp::Div => a.div(b),
      BinaryOp::Min => a.min(b),
      BinaryOp::Max => a.max(b),
    };
    ctx.wave.write_vgpr(dst, lane, result.to_bits());
  }

  Ok(())
}

fn binary_op_dispatch(ctx: &mut Ctx, dtype: DataType, op: BinaryOp) -> ExecResult {
  match dtype {
    DataType::F16 => binary_op_with(ctx, op, read_operand_f16),
    DataType::BF16 => binary_op_with(ctx, op, read_operand_bf16),
    DataType::F32 => binary_op_with(ctx, op, read_operand_f32),
    DataType::I8 => binary_op_with(ctx, op, |ctx, operand, lane| read_operand_int::<i8>(ctx, operand, lane)),
    DataType::I16 => binary_op_with(ctx, op, |ctx, operand, lane| read_operand_int::<i16>(ctx, operand, lane)),
    DataType::I32 => binary_op_with(ctx, op, |ctx, operand, lane| read_operand_int::<i32>(ctx, operand, lane)),
    DataType::U8 => binary_op_with(ctx, op, |ctx, operand, lane| read_operand_int::<u8>(ctx, operand, lane)),
    DataType::U16 => binary_op_with(ctx, op, |ctx, operand, lane| read_operand_int::<u16>(ctx, operand, lane)),
    DataType::U32 => binary_op_with(ctx, op, |ctx, operand, lane| read_operand_int::<u32>(ctx, operand, lane)),
    _ => Err(ExecError::Unimplemented("unsupported data type for binary op")),
  }
}

fn cmp_f32(op: CmpOp, a: f32, b: f32) -> bool {
  let ordered = !(a.is_nan() || b.is_nan());
  match op {
    CmpOp::Eq => ordered && a == b,
    CmpOp::Ne => ordered && a != b,
    CmpOp::Lt => ordered && a < b,
    CmpOp::Le => ordered && a <= b,
    CmpOp::Gt => ordered && a > b,
    CmpOp::Ge => ordered && a >= b,
    CmpOp::Lg => ordered && a != b,
    CmpOp::Neq => !ordered || a != b,
    CmpOp::Nlt => !ordered || !(a < b),
    CmpOp::Nle => !ordered || !(a <= b),
    CmpOp::Ngt => !ordered || !(a > b),
    CmpOp::Nge => !ordered || !(a >= b),
    CmpOp::Nlg => !ordered || a == b,
    CmpOp::O => ordered,
    CmpOp::U => !ordered,
  }
}

fn cmp_op_with<F: Fn(&Ctx, &DecodedOperand, usize) -> f32>(
  ctx: &mut Ctx,
  op: CmpOp,
  update_exec: bool,
  read_operand: F,
) -> ExecResult {
  let src1 = ctx.inst.operands[1].clone();
  let src2 = ctx.inst.operands[2].clone();
  let mut vcc = 0u64;

  for lane in 0..ctx.wave.wave_lanes() {
    if !ctx.wave.is_lane_active(lane) {
      continue;
    }
    let a = read_operand(ctx, &src1, lane);
    let b = read_operand(ctx, &src2, lane);
    if cmp_f32(op, a, b) {
      vcc |= 1u64 << lane;
    }
  }

  ctx.wave.write_vcc(vcc);
  if update_exec {
    ctx.wave.set_exec(ctx.wave.exec_mask() & vcc);
  }
  Ok(())
}

fn cmp_op_int_with<T: NumericType + PartialOrd + PartialEq>(
  ctx: &mut Ctx,
  op: CmpOp,
  update_exec: bool,
) -> ExecResult {
  let src1 = ctx.inst.operands[1].clone();
  let src2 = ctx.inst.operands[2].clone();
  let mut vcc = 0u64;

  for lane in 0..ctx.wave.wave_lanes() {
    if !ctx.wave.is_lane_active(lane) {
      continue;
    }
    let a = read_operand_int::<T>(ctx, &src1, lane);
    let b = read_operand_int::<T>(ctx, &src2, lane);
    let result = match op {
      CmpOp::Eq => a == b,
      CmpOp::Ne => a != b,
      CmpOp::Lt => a < b,
      CmpOp::Le => a <= b,
      CmpOp::Gt => a > b,
      CmpOp::Ge => a >= b,
      _ => return Err(ExecError::Unimplemented("unsupported integer compare op")),
    };
    if result {
      vcc |= 1u64 << lane;
    }
  }

  ctx.wave.write_vcc(vcc);
  if update_exec {
    ctx.wave.set_exec(ctx.wave.exec_mask() & vcc);
  }
  Ok(())
}

fn cmp_op_dispatch(
  ctx: &mut Ctx,
  dtype: DataType,
  op: CmpOp,
  update_exec: bool,
) -> ExecResult {
  match dtype {
    DataType::F16 => cmp_op_with(ctx, op, update_exec, |ctx, operand, lane| {
      read_operand_f16(ctx, operand, lane).to_f32()
    }),
    DataType::BF16 => cmp_op_with(ctx, op, update_exec, |ctx, operand, lane| {
      read_operand_bf16(ctx, operand, lane).to_f32()
    }),
    DataType::F32 => cmp_op_with(ctx, op, update_exec, |ctx, operand, lane| {
      read_operand_f32(ctx, operand, lane)
    }),
    DataType::I8 => cmp_op_int_with::<i8>(ctx, op, update_exec),
    DataType::I16 => cmp_op_int_with::<i16>(ctx, op, update_exec),
    DataType::I32 => cmp_op_int_with::<i32>(ctx, op, update_exec),
    DataType::U8 => cmp_op_int_with::<u8>(ctx, op, update_exec),
    DataType::U16 => cmp_op_int_with::<u16>(ctx, op, update_exec),
    DataType::U32 => cmp_op_int_with::<u32>(ctx, op, update_exec),
    _ => Err(ExecError::Unimplemented("unsupported data type for compare op")),
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::isa::types::InstructionCommonDef;
  use crate::parse_instruction::SpecialRegister;
  use crate::sim::{DecodedInst, GlobalAlloc, LDS};
  use crate::wave::WaveState;
  use crate::WaveSize;

  static COMMON_DEF: InstructionCommonDef = InstructionCommonDef {
    name: "test",
    args: &[],
    dual_args: &[],
    supports_modifiers: false,
  };

  fn new_alloc(size: usize) -> GlobalAlloc {
    GlobalAlloc {
      memory: vec![0u8; size].into_boxed_slice(),
      next: 0,
    }
  }

  #[test]
  fn test_binary_op_f32_with_immediate() {
    let operands = vec![
      DecodedOperand::Vgpr(2),
      DecodedOperand::Vgpr(0),
      DecodedOperand::ImmF32(2.0),
    ];
    let decoded = DecodedInst {
      name: "v_add_f32".to_string(),
      def: &COMMON_DEF,
      line_num: 1,
      operands,
      dual: None,
    };

    let mut wave = WaveState::new(WaveSize::Wave32, 4, 0xFFFF_FFFF).unwrap();
    wave.write_vgpr(0, 0, 1.25f32.to_bits());
    let mut lds = LDS::new(0);
    let mut global_mem = new_alloc(64);

    let mut ctx = Ctx {
      wave: &mut wave,
      lds: &mut lds,
      global_mem: &mut global_mem,
      inst: &decoded,
    };

    run_typed_v(&mut ctx, "v_add_f32", TypedVHandler::Binary { dtype: DataType::F32, op: BinaryOp::Add }).unwrap();
    let result = f32::from_bits(ctx.wave.read_vgpr(2, 0));
    assert!((result - 3.25).abs() < 0.0001);
  }

  #[test]
  fn test_cmp_eq_f32_sets_vcc() {
    let operands = vec![
      DecodedOperand::SpecialReg(SpecialRegister::Vcc),
      DecodedOperand::Vgpr(0),
      DecodedOperand::Vgpr(1),
    ];
    let decoded = DecodedInst {
      name: "v_cmp_eq_f32".to_string(),
      def: &COMMON_DEF,
      line_num: 1,
      operands,
      dual: None,
    };

    let mut wave = WaveState::new(WaveSize::Wave32, 4, 0b11).unwrap();
    wave.write_vgpr(0, 0, 1.0f32.to_bits());
    wave.write_vgpr(0, 1, 2.0f32.to_bits());
    wave.write_vgpr(1, 0, 1.0f32.to_bits());
    wave.write_vgpr(1, 1, 1.0f32.to_bits());
    let mut lds = LDS::new(0);
    let mut global_mem = new_alloc(64);

    let mut ctx = Ctx {
      wave: &mut wave,
      lds: &mut lds,
      global_mem: &mut global_mem,
      inst: &decoded,
    };

    run_typed_v(&mut ctx, "v_cmp_eq_f32", TypedVHandler::Cmp { dtype: DataType::F32, op: CmpOp::Eq, update_exec: false }).unwrap();
    assert_eq!(ctx.wave.read_vcc(), 0b01);
  }

  #[test]
  fn test_cmpx_eq_updates_exec() {
    let operands = vec![
      DecodedOperand::SpecialReg(SpecialRegister::Vcc),
      DecodedOperand::Vgpr(0),
      DecodedOperand::Vgpr(1),
    ];
    let decoded = DecodedInst {
      name: "v_cmpx_eq_f32".to_string(),
      def: &COMMON_DEF,
      line_num: 1,
      operands,
      dual: None,
    };

    let mut wave = WaveState::new(WaveSize::Wave32, 4, 0b11).unwrap();
    wave.write_vgpr(0, 0, 1.0f32.to_bits());
    wave.write_vgpr(0, 1, 2.0f32.to_bits());
    wave.write_vgpr(1, 0, 1.0f32.to_bits());
    wave.write_vgpr(1, 1, 1.0f32.to_bits());
    let mut lds = LDS::new(0);
    let mut global_mem = new_alloc(64);

    let mut ctx = Ctx {
      wave: &mut wave,
      lds: &mut lds,
      global_mem: &mut global_mem,
      inst: &decoded,
    };

    run_typed_v(&mut ctx, "v_cmpx_eq_f32", TypedVHandler::Cmp { dtype: DataType::F32, op: CmpOp::Eq, update_exec: true }).unwrap();
    assert_eq!(ctx.wave.read_vcc(), 0b01);
    assert_eq!(ctx.wave.exec_mask(), 0b01);
  }
}
