use crate::sim::{Ctx, ExecError, ExecResult};

// Control Flow Instructions

pub fn s_barrier(ctx: &mut Ctx) -> ExecResult {
  let _ = ctx;
  Err(ExecError::Unimplemented("s_barrier"))
}

pub fn s_cbranch_cdbgsys(ctx: &mut Ctx) -> ExecResult {
  let _ = ctx;
  Err(ExecError::Unimplemented("s_cbranch_cdbgsys"))
}

pub fn s_cbranch_cdbgsys_and_user(ctx: &mut Ctx) -> ExecResult {
  let _ = ctx;
  Err(ExecError::Unimplemented("s_cbranch_cdbgsys_and_user"))
}

pub fn s_cbranch_cdbgsys_or_user(ctx: &mut Ctx) -> ExecResult {
  let _ = ctx;
  Err(ExecError::Unimplemented("s_cbranch_cdbgsys_or_user"))
}

pub fn s_cbranch_cdbguser(ctx: &mut Ctx) -> ExecResult {
  let _ = ctx;
  Err(ExecError::Unimplemented("s_cbranch_cdbguser"))
}

pub fn s_endpgm_ordered_ps_done(ctx: &mut Ctx) -> ExecResult {
  let _ = ctx;
  Err(ExecError::Unimplemented("s_endpgm_ordered_ps_done"))
}

pub fn s_waitcnt(ctx: &mut Ctx) -> ExecResult {
  let _ = ctx;
  Err(ExecError::Unimplemented("s_waitcnt"))
}

pub fn s_waitcnt_depctr(ctx: &mut Ctx) -> ExecResult {
  let _ = ctx;
  Err(ExecError::Unimplemented("s_waitcnt_depctr"))
}

pub fn s_waitcnt_expcnt(ctx: &mut Ctx) -> ExecResult {
  let _ = ctx;
  Err(ExecError::Unimplemented("s_waitcnt_expcnt"))
}

pub fn s_waitcnt_lgkmcnt(ctx: &mut Ctx) -> ExecResult {
  let _ = ctx;
  Err(ExecError::Unimplemented("s_waitcnt_lgkmcnt"))
}

pub fn s_waitcnt_vmcnt(ctx: &mut Ctx) -> ExecResult {
  let _ = ctx;
  Err(ExecError::Unimplemented("s_waitcnt_vmcnt"))
}

pub fn s_waitcnt_vscnt(ctx: &mut Ctx) -> ExecResult {
  let _ = ctx;
  Err(ExecError::Unimplemented("s_waitcnt_vscnt"))
}

