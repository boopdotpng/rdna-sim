use crate::sim::{DecodedInst, ExecContext, ExecError, ExecResult};

// Control Flow Instructions

pub fn s_barrier_signal(ctx: &mut ExecContext, inst: &DecodedInst) -> ExecResult {
  let _ = (ctx, inst);
  Err(ExecError::Unimplemented("s_barrier_signal"))
}

pub fn s_barrier_signal_isfirst(ctx: &mut ExecContext, inst: &DecodedInst) -> ExecResult {
  let _ = (ctx, inst);
  Err(ExecError::Unimplemented("s_barrier_signal_isfirst"))
}

pub fn s_barrier_wait(ctx: &mut ExecContext, inst: &DecodedInst) -> ExecResult {
  let _ = (ctx, inst);
  Err(ExecError::Unimplemented("s_barrier_wait"))
}

pub fn s_sleep_var(ctx: &mut ExecContext, inst: &DecodedInst) -> ExecResult {
  let _ = (ctx, inst);
  Err(ExecError::Unimplemented("s_sleep_var"))
}

pub fn s_wait_alu(ctx: &mut ExecContext, inst: &DecodedInst) -> ExecResult {
  let _ = (ctx, inst);
  Err(ExecError::Unimplemented("s_wait_alu"))
}

pub fn s_wait_bvhcnt(ctx: &mut ExecContext, inst: &DecodedInst) -> ExecResult {
  let _ = (ctx, inst);
  Err(ExecError::Unimplemented("s_wait_bvhcnt"))
}

pub fn s_wait_dscnt(ctx: &mut ExecContext, inst: &DecodedInst) -> ExecResult {
  let _ = (ctx, inst);
  Err(ExecError::Unimplemented("s_wait_dscnt"))
}

pub fn s_wait_expcnt(ctx: &mut ExecContext, inst: &DecodedInst) -> ExecResult {
  let _ = (ctx, inst);
  Err(ExecError::Unimplemented("s_wait_expcnt"))
}

pub fn s_wait_kmcnt(ctx: &mut ExecContext, inst: &DecodedInst) -> ExecResult {
  let _ = (ctx, inst);
  Err(ExecError::Unimplemented("s_wait_kmcnt"))
}

pub fn s_wait_samplecnt(ctx: &mut ExecContext, inst: &DecodedInst) -> ExecResult {
  let _ = (ctx, inst);
  Err(ExecError::Unimplemented("s_wait_samplecnt"))
}

