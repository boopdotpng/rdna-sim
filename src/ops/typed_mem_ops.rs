use crate::isa::types::DataType;
use crate::sim::{Ctx, ExecError, ExecResult};

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum TypedMemHandler {
  Unknown { dtype: DataType },
}

pub fn run_typed_mem(ctx: &mut Ctx, name: &'static str, _handler: TypedMemHandler) -> ExecResult {
  let _ = ctx;
  Err(ExecError::Unimplemented(name))
}
