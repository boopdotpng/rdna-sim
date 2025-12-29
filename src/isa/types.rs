#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum ArgKind {
  Sgpr,
  Vgpr,
  RegOrImm,
  Imm,
  Mem,
  Label,
  Special,
  Unknown,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum DataType {
  None,
  Any,
  Unknown,
  B1,
  B8,
  B16,
  B32,
  B64,
  B96,
  B128,
  F16,
  F32,
  F64,
  BF16,
  I8,
  I16,
  I24,
  I32,
  I64,
  U8,
  U16,
  U24,
  U32,
  U64,
  M64,
  Pk2B16,
  Pk2BF16,
  Pk2F16,
  Pk2I16,
  Pk2U16,
  Pk2U8,
  Pk4B8,
  Pk4IU8,
  Pk4U8,
  Pk8IU4,
  Pk8U4,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct ArgSpec {
  pub kind: ArgKind,
  pub data_type: DataType,
  pub width: u16,
}
