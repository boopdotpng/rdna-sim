use crate::ops::typed_mem_ops::{run_typed_mem, TypedMemHandler};
use crate::ops::typed_s_ops::{run_typed_s, TypedSHandler};
use crate::ops::typed_v_ops::{run_typed_v, TypedVHandler};
use crate::sim::{Ctx, ExecResult};

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum TypedHandler {
  V(TypedVHandler),
  S(TypedSHandler),
  Mem(TypedMemHandler),
}

pub fn run_typed(ctx: &mut Ctx, name: &'static str, handler: TypedHandler) -> ExecResult {
  match handler {
    TypedHandler::V(v) => run_typed_v(ctx, name, v),
    TypedHandler::S(s) => run_typed_s(ctx, name, s),
    TypedHandler::Mem(m) => run_typed_mem(ctx, name, m),
  }
}
