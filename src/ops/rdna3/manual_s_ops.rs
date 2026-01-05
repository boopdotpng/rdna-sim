use crate::sim::{Ctx, ExecError, ExecResult};

pub fn s_addc_u32(ctx: &mut Ctx) -> ExecResult {
  let _ = ctx;
  Err(ExecError::Unimplemented("s_addc_u32"))
}

pub fn s_addk_i32(ctx: &mut Ctx) -> ExecResult {
  let _ = ctx;
  Err(ExecError::Unimplemented("s_addk_i32"))
}

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

pub fn s_gl1_inv(ctx: &mut Ctx) -> ExecResult {
  let _ = ctx;
  Err(ExecError::Unimplemented("s_gl1_inv"))
}

pub fn s_set_inst_prefetch_distance(ctx: &mut Ctx) -> ExecResult {
  let _ = ctx;
  Err(ExecError::Unimplemented("s_set_inst_prefetch_distance"))
}

pub fn s_subb_u32(ctx: &mut Ctx) -> ExecResult {
  let _ = ctx;
  Err(ExecError::Unimplemented("s_subb_u32"))
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
