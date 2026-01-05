use crate::isa::types::DataType;
use crate::sim::{Ctx, ExecError, ExecResult};

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum TypedSHandler {
  Unknown { dtype: DataType },
}

pub fn run_typed_s(ctx: &mut Ctx, name: &'static str, _handler: TypedSHandler) -> ExecResult {
  let _ = ctx;
  Err(ExecError::Unimplemented(name))
}
