use crate::isa::InstructionCommonDef;
use crate::{Program, WaveState};

#[derive(Debug)]
pub enum ExecError {
    Unimplemented(&'static str),
}

pub type ExecResult = Result<(), ExecError>;

pub struct DecodedInst;

pub struct ExecContext<'a> {
    pub wave: &'a mut WaveState,
    pub program: &'a mut Program,
}

pub type Handler = fn(&mut ExecContext, &DecodedInst) -> ExecResult;

pub fn dispatch(
    arch_ops: &[(&'static str, Handler)],
    base_ops: &[(&'static str, Handler)],
    def: &InstructionCommonDef,
    ctx: &mut ExecContext,
    decoded: &DecodedInst,
) -> ExecResult {
    if let Ok(idx) = arch_ops.binary_search_by(|(name, _)| name.cmp(&def.name)) {
        return (arch_ops[idx].1)(ctx, decoded);
    }
    if let Ok(idx) = base_ops.binary_search_by(|(name, _)| name.cmp(&def.name)) {
        return (base_ops[idx].1)(ctx, decoded);
    }
    Err(ExecError::Unimplemented(def.name))
}
