use crate::sim::{DecodedInst, ExecContext, ExecError, ExecResult};

// Control Flow Instructions

pub fn s_branch(ctx: &mut ExecContext, inst: &DecodedInst) -> ExecResult {
  let _ = (ctx, inst);
  Err(ExecError::Unimplemented("s_branch"))
}

pub fn s_call_b64(ctx: &mut ExecContext, inst: &DecodedInst) -> ExecResult {
  let _ = (ctx, inst);
  Err(ExecError::Unimplemented("s_call_b64"))
}

pub fn s_cbranch_execnz(ctx: &mut ExecContext, inst: &DecodedInst) -> ExecResult {
  let _ = (ctx, inst);
  Err(ExecError::Unimplemented("s_cbranch_execnz"))
}

pub fn s_cbranch_execz(ctx: &mut ExecContext, inst: &DecodedInst) -> ExecResult {
  let _ = (ctx, inst);
  Err(ExecError::Unimplemented("s_cbranch_execz"))
}

pub fn s_cbranch_scc0(ctx: &mut ExecContext, inst: &DecodedInst) -> ExecResult {
  let _ = (ctx, inst);
  Err(ExecError::Unimplemented("s_cbranch_scc0"))
}

pub fn s_cbranch_scc1(ctx: &mut ExecContext, inst: &DecodedInst) -> ExecResult {
  let _ = (ctx, inst);
  Err(ExecError::Unimplemented("s_cbranch_scc1"))
}

pub fn s_cbranch_vccnz(ctx: &mut ExecContext, inst: &DecodedInst) -> ExecResult {
  let _ = (ctx, inst);
  Err(ExecError::Unimplemented("s_cbranch_vccnz"))
}

pub fn s_cbranch_vccz(ctx: &mut ExecContext, inst: &DecodedInst) -> ExecResult {
  let _ = (ctx, inst);
  Err(ExecError::Unimplemented("s_cbranch_vccz"))
}

pub fn s_endpgm(ctx: &mut ExecContext, inst: &DecodedInst) -> ExecResult {
  let _ = (ctx, inst);
  Err(ExecError::Unimplemented("s_endpgm"))
}

pub fn s_endpgm_saved(ctx: &mut ExecContext, inst: &DecodedInst) -> ExecResult {
  let _ = (ctx, inst);
  Err(ExecError::Unimplemented("s_endpgm_saved"))
}

pub fn s_getpc_b64(ctx: &mut ExecContext, inst: &DecodedInst) -> ExecResult {
  let _ = (ctx, inst);
  Err(ExecError::Unimplemented("s_getpc_b64"))
}

pub fn s_rfe_b64(ctx: &mut ExecContext, inst: &DecodedInst) -> ExecResult {
  let _ = (ctx, inst);
  Err(ExecError::Unimplemented("s_rfe_b64"))
}

pub fn s_sendmsg(ctx: &mut ExecContext, inst: &DecodedInst) -> ExecResult {
  let _ = (ctx, inst);
  Err(ExecError::Unimplemented("s_sendmsg"))
}

pub fn s_sendmsg_rtn_b32(ctx: &mut ExecContext, inst: &DecodedInst) -> ExecResult {
  let _ = (ctx, inst);
  Err(ExecError::Unimplemented("s_sendmsg_rtn_b32"))
}

pub fn s_sendmsg_rtn_b64(ctx: &mut ExecContext, inst: &DecodedInst) -> ExecResult {
  let _ = (ctx, inst);
  Err(ExecError::Unimplemented("s_sendmsg_rtn_b64"))
}

pub fn s_sendmsghalt(ctx: &mut ExecContext, inst: &DecodedInst) -> ExecResult {
  let _ = (ctx, inst);
  Err(ExecError::Unimplemented("s_sendmsghalt"))
}

pub fn s_sethalt(ctx: &mut ExecContext, inst: &DecodedInst) -> ExecResult {
  let _ = (ctx, inst);
  Err(ExecError::Unimplemented("s_sethalt"))
}

pub fn s_setkill(ctx: &mut ExecContext, inst: &DecodedInst) -> ExecResult {
  let _ = (ctx, inst);
  Err(ExecError::Unimplemented("s_setkill"))
}

pub fn s_setpc_b64(ctx: &mut ExecContext, inst: &DecodedInst) -> ExecResult {
  let _ = (ctx, inst);
  Err(ExecError::Unimplemented("s_setpc_b64"))
}

pub fn s_setprio(ctx: &mut ExecContext, inst: &DecodedInst) -> ExecResult {
  let _ = (ctx, inst);
  Err(ExecError::Unimplemented("s_setprio"))
}

pub fn s_sleep(ctx: &mut ExecContext, inst: &DecodedInst) -> ExecResult {
  let _ = (ctx, inst);
  Err(ExecError::Unimplemented("s_sleep"))
}

pub fn s_swappc_b64(ctx: &mut ExecContext, inst: &DecodedInst) -> ExecResult {
  let _ = (ctx, inst);
  Err(ExecError::Unimplemented("s_swappc_b64"))
}

pub fn s_trap(ctx: &mut ExecContext, inst: &DecodedInst) -> ExecResult {
  let _ = (ctx, inst);
  Err(ExecError::Unimplemented("s_trap"))
}

pub fn s_wait_event(ctx: &mut ExecContext, inst: &DecodedInst) -> ExecResult {
  let _ = (ctx, inst);
  Err(ExecError::Unimplemented("s_wait_event"))
}

pub fn s_wait_idle(ctx: &mut ExecContext, inst: &DecodedInst) -> ExecResult {
  let _ = (ctx, inst);
  Err(ExecError::Unimplemented("s_wait_idle"))
}

pub fn s_wakeup(ctx: &mut ExecContext, inst: &DecodedInst) -> ExecResult {
  let _ = (ctx, inst);
  Err(ExecError::Unimplemented("s_wakeup"))
}

// Execution Mask Management

pub fn s_and_not0_saveexec_b32(ctx: &mut ExecContext, inst: &DecodedInst) -> ExecResult {
  let _ = (ctx, inst);
  Err(ExecError::Unimplemented("s_and_not0_saveexec_b32"))
}

pub fn s_and_not0_saveexec_b64(ctx: &mut ExecContext, inst: &DecodedInst) -> ExecResult {
  let _ = (ctx, inst);
  Err(ExecError::Unimplemented("s_and_not0_saveexec_b64"))
}

pub fn s_and_not0_wrexec_b32(ctx: &mut ExecContext, inst: &DecodedInst) -> ExecResult {
  let _ = (ctx, inst);
  Err(ExecError::Unimplemented("s_and_not0_wrexec_b32"))
}

pub fn s_and_not0_wrexec_b64(ctx: &mut ExecContext, inst: &DecodedInst) -> ExecResult {
  let _ = (ctx, inst);
  Err(ExecError::Unimplemented("s_and_not0_wrexec_b64"))
}

pub fn s_and_not1_saveexec_b32(ctx: &mut ExecContext, inst: &DecodedInst) -> ExecResult {
  let _ = (ctx, inst);
  Err(ExecError::Unimplemented("s_and_not1_saveexec_b32"))
}

pub fn s_and_not1_saveexec_b64(ctx: &mut ExecContext, inst: &DecodedInst) -> ExecResult {
  let _ = (ctx, inst);
  Err(ExecError::Unimplemented("s_and_not1_saveexec_b64"))
}

pub fn s_and_not1_wrexec_b32(ctx: &mut ExecContext, inst: &DecodedInst) -> ExecResult {
  let _ = (ctx, inst);
  Err(ExecError::Unimplemented("s_and_not1_wrexec_b32"))
}

pub fn s_and_not1_wrexec_b64(ctx: &mut ExecContext, inst: &DecodedInst) -> ExecResult {
  let _ = (ctx, inst);
  Err(ExecError::Unimplemented("s_and_not1_wrexec_b64"))
}

pub fn s_and_saveexec_b32(ctx: &mut ExecContext, inst: &DecodedInst) -> ExecResult {
  let _ = (ctx, inst);
  Err(ExecError::Unimplemented("s_and_saveexec_b32"))
}

pub fn s_and_saveexec_b64(ctx: &mut ExecContext, inst: &DecodedInst) -> ExecResult {
  let _ = (ctx, inst);
  Err(ExecError::Unimplemented("s_and_saveexec_b64"))
}

pub fn s_nand_saveexec_b32(ctx: &mut ExecContext, inst: &DecodedInst) -> ExecResult {
  let _ = (ctx, inst);
  Err(ExecError::Unimplemented("s_nand_saveexec_b32"))
}

pub fn s_nand_saveexec_b64(ctx: &mut ExecContext, inst: &DecodedInst) -> ExecResult {
  let _ = (ctx, inst);
  Err(ExecError::Unimplemented("s_nand_saveexec_b64"))
}

pub fn s_nor_saveexec_b32(ctx: &mut ExecContext, inst: &DecodedInst) -> ExecResult {
  let _ = (ctx, inst);
  Err(ExecError::Unimplemented("s_nor_saveexec_b32"))
}

pub fn s_nor_saveexec_b64(ctx: &mut ExecContext, inst: &DecodedInst) -> ExecResult {
  let _ = (ctx, inst);
  Err(ExecError::Unimplemented("s_nor_saveexec_b64"))
}

pub fn s_or_not0_saveexec_b32(ctx: &mut ExecContext, inst: &DecodedInst) -> ExecResult {
  let _ = (ctx, inst);
  Err(ExecError::Unimplemented("s_or_not0_saveexec_b32"))
}

pub fn s_or_not0_saveexec_b64(ctx: &mut ExecContext, inst: &DecodedInst) -> ExecResult {
  let _ = (ctx, inst);
  Err(ExecError::Unimplemented("s_or_not0_saveexec_b64"))
}

pub fn s_or_not1_saveexec_b32(ctx: &mut ExecContext, inst: &DecodedInst) -> ExecResult {
  let _ = (ctx, inst);
  Err(ExecError::Unimplemented("s_or_not1_saveexec_b32"))
}

pub fn s_or_not1_saveexec_b64(ctx: &mut ExecContext, inst: &DecodedInst) -> ExecResult {
  let _ = (ctx, inst);
  Err(ExecError::Unimplemented("s_or_not1_saveexec_b64"))
}

pub fn s_or_saveexec_b32(ctx: &mut ExecContext, inst: &DecodedInst) -> ExecResult {
  let _ = (ctx, inst);
  Err(ExecError::Unimplemented("s_or_saveexec_b32"))
}

pub fn s_or_saveexec_b64(ctx: &mut ExecContext, inst: &DecodedInst) -> ExecResult {
  let _ = (ctx, inst);
  Err(ExecError::Unimplemented("s_or_saveexec_b64"))
}

pub fn s_xnor_saveexec_b32(ctx: &mut ExecContext, inst: &DecodedInst) -> ExecResult {
  let _ = (ctx, inst);
  Err(ExecError::Unimplemented("s_xnor_saveexec_b32"))
}

pub fn s_xnor_saveexec_b64(ctx: &mut ExecContext, inst: &DecodedInst) -> ExecResult {
  let _ = (ctx, inst);
  Err(ExecError::Unimplemented("s_xnor_saveexec_b64"))
}

pub fn s_xor_saveexec_b32(ctx: &mut ExecContext, inst: &DecodedInst) -> ExecResult {
  let _ = (ctx, inst);
  Err(ExecError::Unimplemented("s_xor_saveexec_b32"))
}

pub fn s_xor_saveexec_b64(ctx: &mut ExecContext, inst: &DecodedInst) -> ExecResult {
  let _ = (ctx, inst);
  Err(ExecError::Unimplemented("s_xor_saveexec_b64"))
}

// Quad/Wave Mode Control

pub fn s_quadmask_b32(ctx: &mut ExecContext, inst: &DecodedInst) -> ExecResult {
  let _ = (ctx, inst);
  Err(ExecError::Unimplemented("s_quadmask_b32"))
}

pub fn s_quadmask_b64(ctx: &mut ExecContext, inst: &DecodedInst) -> ExecResult {
  let _ = (ctx, inst);
  Err(ExecError::Unimplemented("s_quadmask_b64"))
}

pub fn s_wqm_b32(ctx: &mut ExecContext, inst: &DecodedInst) -> ExecResult {
  let _ = (ctx, inst);
  Err(ExecError::Unimplemented("s_wqm_b32"))
}

pub fn s_wqm_b64(ctx: &mut ExecContext, inst: &DecodedInst) -> ExecResult {
  let _ = (ctx, inst);
  Err(ExecError::Unimplemented("s_wqm_b64"))
}

// System State Management

pub fn s_dcache_inv(ctx: &mut ExecContext, inst: &DecodedInst) -> ExecResult {
  let _ = (ctx, inst);
  Err(ExecError::Unimplemented("s_dcache_inv"))
}

pub fn s_decperflevel(ctx: &mut ExecContext, inst: &DecodedInst) -> ExecResult {
  let _ = (ctx, inst);
  Err(ExecError::Unimplemented("s_decperflevel"))
}

pub fn s_delay_alu(ctx: &mut ExecContext, inst: &DecodedInst) -> ExecResult {
  let _ = (ctx, inst);
  Err(ExecError::Unimplemented("s_delay_alu"))
}

pub fn s_denorm_mode(ctx: &mut ExecContext, inst: &DecodedInst) -> ExecResult {
  let _ = (ctx, inst);
  Err(ExecError::Unimplemented("s_denorm_mode"))
}

pub fn s_icache_inv(ctx: &mut ExecContext, inst: &DecodedInst) -> ExecResult {
  let _ = (ctx, inst);
  Err(ExecError::Unimplemented("s_icache_inv"))
}

pub fn s_incperflevel(ctx: &mut ExecContext, inst: &DecodedInst) -> ExecResult {
  let _ = (ctx, inst);
  Err(ExecError::Unimplemented("s_incperflevel"))
}

pub fn s_round_mode(ctx: &mut ExecContext, inst: &DecodedInst) -> ExecResult {
  let _ = (ctx, inst);
  Err(ExecError::Unimplemented("s_round_mode"))
}

// Special Register Access

pub fn s_getreg_b32(ctx: &mut ExecContext, inst: &DecodedInst) -> ExecResult {
  let _ = (ctx, inst);
  Err(ExecError::Unimplemented("s_getreg_b32"))
}

pub fn s_setreg_b32(ctx: &mut ExecContext, inst: &DecodedInst) -> ExecResult {
  let _ = (ctx, inst);
  Err(ExecError::Unimplemented("s_setreg_b32"))
}

pub fn s_setreg_imm32_b32(ctx: &mut ExecContext, inst: &DecodedInst) -> ExecResult {
  let _ = (ctx, inst);
  Err(ExecError::Unimplemented("s_setreg_imm32_b32"))
}

// Relative Addressing

pub fn s_movreld_b32(ctx: &mut ExecContext, inst: &DecodedInst) -> ExecResult {
  let _ = (ctx, inst);
  Err(ExecError::Unimplemented("s_movreld_b32"))
}

pub fn s_movreld_b64(ctx: &mut ExecContext, inst: &DecodedInst) -> ExecResult {
  let _ = (ctx, inst);
  Err(ExecError::Unimplemented("s_movreld_b64"))
}

pub fn s_movrels_b32(ctx: &mut ExecContext, inst: &DecodedInst) -> ExecResult {
  let _ = (ctx, inst);
  Err(ExecError::Unimplemented("s_movrels_b32"))
}

pub fn s_movrels_b64(ctx: &mut ExecContext, inst: &DecodedInst) -> ExecResult {
  let _ = (ctx, inst);
  Err(ExecError::Unimplemented("s_movrels_b64"))
}

pub fn s_movrelsd_2_b32(ctx: &mut ExecContext, inst: &DecodedInst) -> ExecResult {
  let _ = (ctx, inst);
  Err(ExecError::Unimplemented("s_movrelsd_2_b32"))
}

// Debug/Trace/Utility

pub fn s_clause(ctx: &mut ExecContext, inst: &DecodedInst) -> ExecResult {
  let _ = (ctx, inst);
  Err(ExecError::Unimplemented("s_clause"))
}

pub fn s_code_end(ctx: &mut ExecContext, inst: &DecodedInst) -> ExecResult {
  let _ = (ctx, inst);
  Err(ExecError::Unimplemented("s_code_end"))
}

pub fn s_nop(ctx: &mut ExecContext, inst: &DecodedInst) -> ExecResult {
  let _ = (ctx, inst);
  Err(ExecError::Unimplemented("s_nop"))
}

pub fn s_ttracedata(ctx: &mut ExecContext, inst: &DecodedInst) -> ExecResult {
  let _ = (ctx, inst);
  Err(ExecError::Unimplemented("s_ttracedata"))
}

pub fn s_ttracedata_imm(ctx: &mut ExecContext, inst: &DecodedInst) -> ExecResult {
  let _ = (ctx, inst);
  Err(ExecError::Unimplemented("s_ttracedata_imm"))
}

pub fn s_version(ctx: &mut ExecContext, inst: &DecodedInst) -> ExecResult {
  let _ = (ctx, inst);
  Err(ExecError::Unimplemented("s_version"))
}

