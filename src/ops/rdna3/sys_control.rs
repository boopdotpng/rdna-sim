use crate::sim::{DecodedInst, ExecContext, ExecError, ExecResult};

// Control Flow Instructions

pub fn s_barrier(ctx: &mut ExecContext, inst: &DecodedInst) -> ExecResult {
  let _ = (ctx, inst);
  Err(ExecError::Unimplemented("s_barrier"))
}

pub fn s_cbranch_cdbgsys(ctx: &mut ExecContext, inst: &DecodedInst) -> ExecResult {
  let _ = (ctx, inst);
  Err(ExecError::Unimplemented("s_cbranch_cdbgsys"))
}

pub fn s_cbranch_cdbgsys_and_user(ctx: &mut ExecContext, inst: &DecodedInst) -> ExecResult {
  let _ = (ctx, inst);
  Err(ExecError::Unimplemented("s_cbranch_cdbgsys_and_user"))
}

pub fn s_cbranch_cdbgsys_or_user(ctx: &mut ExecContext, inst: &DecodedInst) -> ExecResult {
  let _ = (ctx, inst);
  Err(ExecError::Unimplemented("s_cbranch_cdbgsys_or_user"))
}

pub fn s_cbranch_cdbguser(ctx: &mut ExecContext, inst: &DecodedInst) -> ExecResult {
  let _ = (ctx, inst);
  Err(ExecError::Unimplemented("s_cbranch_cdbguser"))
}

pub fn s_endpgm_ordered_ps_done(ctx: &mut ExecContext, inst: &DecodedInst) -> ExecResult {
  let _ = (ctx, inst);
  Err(ExecError::Unimplemented("s_endpgm_ordered_ps_done"))
}

pub fn s_waitcnt(ctx: &mut ExecContext, inst: &DecodedInst) -> ExecResult {
  let _ = (ctx, inst);
  Err(ExecError::Unimplemented("s_waitcnt"))
}

pub fn s_waitcnt_depctr(ctx: &mut ExecContext, inst: &DecodedInst) -> ExecResult {
  let _ = (ctx, inst);
  Err(ExecError::Unimplemented("s_waitcnt_depctr"))
}

pub fn s_waitcnt_expcnt(ctx: &mut ExecContext, inst: &DecodedInst) -> ExecResult {
  let _ = (ctx, inst);
  Err(ExecError::Unimplemented("s_waitcnt_expcnt"))
}

pub fn s_waitcnt_lgkmcnt(ctx: &mut ExecContext, inst: &DecodedInst) -> ExecResult {
  let _ = (ctx, inst);
  Err(ExecError::Unimplemented("s_waitcnt_lgkmcnt"))
}

pub fn s_waitcnt_vmcnt(ctx: &mut ExecContext, inst: &DecodedInst) -> ExecResult {
  let _ = (ctx, inst);
  Err(ExecError::Unimplemented("s_waitcnt_vmcnt"))
}

pub fn s_waitcnt_vscnt(ctx: &mut ExecContext, inst: &DecodedInst) -> ExecResult {
  let _ = (ctx, inst);
  Err(ExecError::Unimplemented("s_waitcnt_vscnt"))
}

