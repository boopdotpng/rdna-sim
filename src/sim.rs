use half::bf16;

use crate::isa::types::InstructionCommonDef;
use crate::parse_instruction::SpecialRegister;
use crate::wave::WaveState;
use crate::Program;

pub trait MemoryValue: Sized {
    const SIZE: usize;
    fn from_le_bytes(bytes: &[u8]) -> Self;
}

impl MemoryValue for u8 {
    const SIZE: usize = 1;
    fn from_le_bytes(bytes: &[u8]) -> Self {
        bytes[0]
    }
}

impl MemoryValue for i8 {
    const SIZE: usize = 1;
    fn from_le_bytes(bytes: &[u8]) -> Self {
        bytes[0] as i8
    }
}

impl MemoryValue for u16 {
    const SIZE: usize = 2;
    fn from_le_bytes(bytes: &[u8]) -> Self {
        u16::from_le_bytes([bytes[0], bytes[1]])
    }
}

impl MemoryValue for i16 {
    const SIZE: usize = 2;
    fn from_le_bytes(bytes: &[u8]) -> Self {
        u16::from_le_bytes([bytes[0], bytes[1]]) as i16
    }
}

impl MemoryValue for u32 {
    const SIZE: usize = 4;
    fn from_le_bytes(bytes: &[u8]) -> Self {
        u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
    }
}

impl MemoryValue for i32 {
    const SIZE: usize = 4;
    fn from_le_bytes(bytes: &[u8]) -> Self {
        u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as i32
    }
}

impl MemoryValue for u64 {
    const SIZE: usize = 8;
    fn from_le_bytes(bytes: &[u8]) -> Self {
        u64::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3],
            bytes[4], bytes[5], bytes[6], bytes[7],
        ])
    }
}

impl MemoryValue for i64 {
    const SIZE: usize = 8;
    fn from_le_bytes(bytes: &[u8]) -> Self {
        u64::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3],
            bytes[4], bytes[5], bytes[6], bytes[7],
        ]) as i64
    }
}

impl MemoryValue for f32 {
    const SIZE: usize = 4;
    fn from_le_bytes(bytes: &[u8]) -> Self {
        f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
    }
}

/// Common memory operations trait for both global memory and LDS
pub trait MemoryOps {
    fn memory(&self) -> &[u8];
    fn memory_mut(&mut self) -> &mut [u8];

    fn write(&mut self, addr: u64, data: &[u8]) -> Result<(), String> {
        let start = addr as usize;
        let end = start
            .checked_add(data.len())
            .ok_or_else(|| "memory write overflow".to_string())?;
        if end > self.memory().len() {
            return Err(format!(
                "memory write out of bounds: {}..{} (len {})",
                start,
                end,
                self.memory().len()
            ));
        }
        self.memory_mut()[start..end].copy_from_slice(data);
        Ok(())
    }

    fn write_zeros(&mut self, addr: u64, size: usize) -> Result<(), String> {
        let start = addr as usize;
        let end = start
            .checked_add(size)
            .ok_or_else(|| "memory write overflow".to_string())?;
        if end > self.memory().len() {
            return Err(format!(
                "memory write out of bounds: {}..{} (len {})",
                start,
                end,
                self.memory().len()
            ));
        }
        self.memory_mut()[start..end].fill(0);
        Ok(())
    }

    fn read(&self, addr: u64, size: usize) -> Result<Vec<u8>, String> {
        let start = addr as usize;
        let end = start
            .checked_add(size)
            .ok_or_else(|| "memory read overflow".to_string())?;
        if end > self.memory().len() {
            return Err(format!(
                "memory read out of bounds: {}..{} (len {})",
                start,
                end,
                self.memory().len()
            ));
        }
        Ok(self.memory()[start..end].to_vec())
    }

    fn read_value<T: MemoryValue>(&self, addr: u64) -> Result<T, String> {
        let bytes = self.read(addr, T::SIZE)?;
        Ok(T::from_le_bytes(&bytes))
    }

    fn read_bf16_as_f32(&self, addr: u64) -> Result<f32, String> {
        let bits = self.read_value::<u16>(addr)?;
        Ok(bf16::from_bits(bits).to_f32())
    }

    fn write_bf16_from_f32(&mut self, addr: u64, value: f32) -> Result<(), String> {
        let bytes = bf16::from_f32(value).to_bits().to_le_bytes();
        self.write(addr, &bytes)
    }
}

#[derive(Clone, Debug)]
pub struct GlobalAlloc {
    pub(crate) memory: Box<[u8]>, // not dynamic. u8 is divisible by all byte-widths used, f32, f16, etc.
    pub(crate) next: usize, // next available address for allocation
}

impl MemoryOps for GlobalAlloc {
    fn memory(&self) -> &[u8] {
        &self.memory
    }

    fn memory_mut(&mut self) -> &mut [u8] {
        &mut self.memory
    }
}

impl GlobalAlloc {
    pub fn alloc(&mut self, size: usize, align: usize) -> Result<u64, String> {
        let align = align.max(1);
        let aligned = (self.next + align - 1) / align * align;
        let end = aligned
            .checked_add(size)
            .ok_or_else(|| "global alloc overflow".to_string())?;
        if end > self.memory.len() {
            return Err(format!(
                "global alloc out of memory: need {}, have {}",
                end,
                self.memory.len()
            ));
        }
        self.next = end;
        Ok(aligned as u64)
    }
}

#[derive(Clone, Debug)]
pub struct KernArg {
  pub base: u64,
  pub size: usize,
}

impl KernArg {
  pub fn new(program: &mut Program, args: &[u64]) -> Result<Self, String> {
    if args.is_empty() {
      return Ok(Self { base: 0, size: 0 });
    }
    let size = args.len() * 8;
    let base = program.global_mem.alloc(size, 8)?;
    for (idx, addr) in args.iter().enumerate() {
      let offset = (idx * 8) as u64;
      program.global_mem.write(base + offset, &addr.to_le_bytes())?;
    }
    Ok(Self { base, size })
  }

  pub fn is_empty(&self) -> bool {
    self.size == 0
  }
}

/// Local Data Store (LDS) - shared memory visible to all threads in a workgroup
/// Allocated and aligned in u32 (4-byte) chunks
#[derive(Clone, Debug)]
pub struct LDS {
    pub(crate) memory: Box<[u8]>,
    pub(crate) next: usize, // next available address for allocation
}

impl MemoryOps for LDS {
    fn memory(&self) -> &[u8] {
        &self.memory
    }

    fn memory_mut(&mut self) -> &mut [u8] {
        &mut self.memory
    }
}

impl LDS {
    pub fn new(size: usize) -> Self {
        Self {
            memory: vec![0u8; size].into_boxed_slice(),
            next: 0,
        }
    }

    /// Allocate LDS memory with 4-byte minimum alignment
    pub fn alloc(&mut self, size: usize, align: usize) -> Result<u64, String> {
        // LDS requires minimum 4-byte alignment
        let align = align.max(4);
        let aligned = (self.next + align - 1) / align * align;
        let end = aligned
            .checked_add(size)
            .ok_or_else(|| "LDS alloc overflow".to_string())?;
        if end > self.memory.len() {
            return Err(format!(
                "LDS alloc out of memory: need {}, have {}",
                end,
                self.memory.len()
            ));
        }
        self.next = end;
        Ok(aligned as u64)
    }
}

pub fn generate_arange(start: i32, end: i32, step: i32) -> Result<Vec<i32>, String> {
    if step == 0 {
        return Err("arange step cannot be 0".to_string());
    }
    if (step > 0 && start > end) || (step < 0 && start < end) {
        return Err("arange step has wrong sign for range".to_string());
    }
    let mut out = Vec::new();
    let mut value = start;
    if step > 0 {
        while value < end {
            out.push(value);
            value += step;
        }
    } else {
        while value > end {
            out.push(value);
            value += step;
        }
    }
    Ok(out)
}

#[derive(Debug)]
pub enum ExecError {
    Unimplemented(&'static str),
    EndProgram,
}

pub type ExecResult = Result<(), ExecError>;

#[derive(Clone, Debug)]
pub struct DecodedInst {
    /// Instruction metadata
    pub name: String,
    pub def: &'static InstructionCommonDef,
    pub line_num: usize,  // Source line number for error reporting

    /// Decoded operand values
    pub operands: Vec<DecodedOperand>,
}

#[derive(Clone, Debug)]
pub enum DecodedOperand {
    // Register references (just the index, not the value)
    Sgpr(u16),
    SgprRange(u16, u16),  // start, end (inclusive)
    Vgpr(u16),
    VgprRange(u16, u16),
    SpecialReg(SpecialRegister),

    // Immediate values (already converted to runtime type)
    ImmU32(u32),
    ImmI32(i32),
    ImmF32(f32),

    // Memory/addressing
    Offset(u32),

    // Flags (for cache policies, addressing modes, etc.)
    Flag(String),

    // Modifiers wrapping other operands
    Negate(Box<DecodedOperand>),
    Abs(Box<DecodedOperand>),
}

/// Unified instruction execution context
/// Bundles wave state, memory, and the current instruction together
pub struct Ctx<'a> {
    pub wave: &'a mut WaveState,
    pub lds: &'a mut LDS,
    pub global_mem: &'a mut GlobalAlloc,
    pub inst: &'a DecodedInst,
}

pub trait VgprValue: Sized {
    fn apply_modifiers(bits: u32, abs: bool, neg: bool) -> Self;
}

impl VgprValue for u32 {
    fn apply_modifiers(bits: u32, abs: bool, neg: bool) -> Self {
        let mut value = bits as i32;
        if abs {
            value = value.wrapping_abs();
        }
        if neg {
            value = value.wrapping_neg();
        }
        value as u32
    }
}

impl VgprValue for i32 {
    fn apply_modifiers(bits: u32, abs: bool, neg: bool) -> Self {
        let mut value = bits as i32;
        if abs {
            value = value.wrapping_abs();
        }
        if neg {
            value = value.wrapping_neg();
        }
        value
    }
}

impl VgprValue for f32 {
    fn apply_modifiers(bits: u32, abs: bool, neg: bool) -> Self {
        let mut value = f32::from_bits(bits);
        if abs {
            value = value.abs();
        }
        if neg {
            value = -value;
        }
        value
    }
}

impl<'a> Ctx<'a> {
    pub fn dst_sgpr(&self) -> usize {
        match &self.inst.operands[0] {
            DecodedOperand::Sgpr(n) => *n as usize,
            _ => panic!("expected Sgpr dest at operand 0"),
        }
    }

    pub fn dst_sgpr_range(&self) -> usize {
        match &self.inst.operands[0] {
            DecodedOperand::SgprRange(start, _) => *start as usize,
            _ => panic!("expected SgprRange dest at operand 0"),
        }
    }

    pub fn dst_vgpr(&self) -> usize {
        match &self.inst.operands[0] {
            DecodedOperand::Vgpr(n) => *n as usize,
            _ => panic!("expected Vgpr dest at operand 0"),
        }
    }

    pub fn dst_vgpr_range(&self) -> usize {
        match &self.inst.operands[0] {
            DecodedOperand::VgprRange(start, _) => *start as usize,
            _ => panic!("expected VgprRange dest at operand 0"),
        }
    }

    pub fn src(&self, idx: usize) -> &DecodedOperand {
        &self.inst.operands[idx]
    }

    fn unpack_vgpr_modifiers(&self, operand: &DecodedOperand) -> (u16, bool, bool) {
        let mut abs = false;
        let mut neg = false;
        let mut current = operand;

        loop {
            match current {
                DecodedOperand::Vgpr(idx) => return (*idx, abs, neg),
                DecodedOperand::Abs(inner) => {
                    abs = true;
                    current = inner;
                }
                DecodedOperand::Negate(inner) => {
                    neg = true;
                    current = inner;
                }
                _ => panic!("expected Vgpr operand"),
            }
        }
    }

    pub fn read_vgpr<T: VgprValue>(&self, operand: &DecodedOperand, lane: usize) -> T {
        let (idx, abs, neg) = self.unpack_vgpr_modifiers(operand);
        let bits = self.wave.read_vgpr(idx as usize, lane);
        T::apply_modifiers(bits, abs, neg)
    }

    pub fn read_vgpr_bf16_as_f32(&self, operand: &DecodedOperand, lane: usize) -> f32 {
        let (idx, abs, neg) = self.unpack_vgpr_modifiers(operand);
        let bits = self.wave.read_vgpr(idx as usize, lane) as u16;
        let mut value = bf16::from_bits(bits).to_f32();
        if abs {
            value = value.abs();
        }
        if neg {
            value = -value;
        }
        value
    }

    pub fn write_vgpr_bf16_from_f32(&mut self, idx: usize, lane: usize, value: f32) {
        let bits = bf16::from_f32(value).to_bits() as u32;
        self.wave.write_vgpr(idx, lane, bits);
    }
}

pub type Handler = fn(&mut Ctx) -> ExecResult;

pub fn dispatch(
    arch_ops: &[(&'static str, Handler)],
    base_ops: &[(&'static str, Handler)],
    def: &InstructionCommonDef,
    wave: &mut WaveState,
    lds: &mut LDS,
    program: &mut Program,
    decoded: &DecodedInst,
) -> ExecResult {
    // Create Ctx with all necessary context
    let mut ctx = Ctx {
        wave,
        lds,
        global_mem: &mut program.global_mem,
        inst: decoded,
    };

    // Call handler with single unified parameter
    if let Ok(idx) = arch_ops.binary_search_by(|(name, _)| name.cmp(&def.name)) {
        return (arch_ops[idx].1)(&mut ctx);
    }
    if let Ok(idx) = base_ops.binary_search_by(|(name, _)| name.cmp(&def.name)) {
        return (base_ops[idx].1)(&mut ctx);
    }
    Err(ExecError::Unimplemented(def.name))
}



#[cfg(test)]
mod tests {
    use super::*;
    use crate::isa::base::lookup_common_normalized;
    use crate::parse_instruction::SpecialRegister;
    use crate::wave::WaveState;
    use crate::WaveSize;

    fn new_alloc(size: usize) -> GlobalAlloc {
        GlobalAlloc {
            memory: vec![0u8; size].into_boxed_slice(),
            next: 0,
        }
    }

    #[test]
    fn test_alloc_basic() {
        let mut alloc = new_alloc(1024);

        // First allocation should start at 0
        let addr = alloc.alloc(16, 1).unwrap();
        assert_eq!(addr, 0);
        assert_eq!(alloc.next, 16);

        // Second allocation should follow immediately
        let addr2 = alloc.alloc(32, 1).unwrap();
        assert_eq!(addr2, 16);
        assert_eq!(alloc.next, 48);
    }

    #[test]
    fn test_alloc_alignment() {
        let mut alloc = new_alloc(1024);

        // Allocate 1 byte with 1-byte alignment
        let addr1 = alloc.alloc(1, 1).unwrap();
        assert_eq!(addr1, 0);
        assert_eq!(alloc.next, 1);

        // Allocate 4 bytes with 4-byte alignment (should align to offset 4)
        let addr2 = alloc.alloc(4, 4).unwrap();
        assert_eq!(addr2, 4);
        assert_eq!(alloc.next, 8);

        // Allocate 1 byte with 1-byte alignment
        let addr3 = alloc.alloc(1, 1).unwrap();
        assert_eq!(addr3, 8);
        assert_eq!(alloc.next, 9);

        // Allocate 8 bytes with 16-byte alignment (should align to offset 16)
        let addr4 = alloc.alloc(8, 16).unwrap();
        assert_eq!(addr4, 16);
        assert_eq!(alloc.next, 24);
    }

    #[test]
    fn test_alloc_zero_alignment() {
        let mut alloc = new_alloc(1024);

        // Zero alignment should be treated as 1
        let addr = alloc.alloc(10, 0).unwrap();
        assert_eq!(addr, 0);
        assert_eq!(alloc.next, 10);
    }

    #[test]
    fn test_alloc_out_of_memory() {
        let mut alloc = new_alloc(100);

        // Allocate most of the space
        alloc.alloc(90, 1).unwrap();

        // This should fail - not enough space left
        let result = alloc.alloc(20, 1);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("out of memory"));
    }

    #[test]
    fn test_alloc_overflow() {
        let mut alloc = new_alloc(100);

        // Allocate something first to move `next` forward
        alloc.alloc(10, 1).unwrap();

        // Now try to allocate a size that would overflow when added to `next`
        let result = alloc.alloc(usize::MAX - 5, 1);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("overflow"));
    }

    #[test]
    fn test_write_basic() {
        let mut alloc = new_alloc(1024);

        let data = vec![1, 2, 3, 4, 5];
        alloc.write(0, &data).unwrap();

        // Verify data was written
        assert_eq!(alloc.memory[0], 1);
        assert_eq!(alloc.memory[1], 2);
        assert_eq!(alloc.memory[2], 3);
        assert_eq!(alloc.memory[3], 4);
        assert_eq!(alloc.memory[4], 5);
    }

    #[test]
    fn test_write_at_offset() {
        let mut alloc = new_alloc(1024);

        let data = vec![10, 20, 30];
        alloc.write(100, &data).unwrap();

        assert_eq!(alloc.memory[100], 10);
        assert_eq!(alloc.memory[101], 20);
        assert_eq!(alloc.memory[102], 30);
    }

    #[test]
    fn test_write_out_of_bounds() {
        let mut alloc = new_alloc(100);

        let data = vec![1, 2, 3, 4, 5];

        // Write that would go past the end
        let result = alloc.write(98, &data);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("out of bounds"));
    }

    #[test]
    fn test_write_overflow() {
        let mut alloc = new_alloc(100);

        let data = vec![1, 2, 3];
        let result = alloc.write(u64::MAX - 1, &data);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("overflow"));
    }

    #[test]
    fn test_write_zeros_overflow() {
        let mut alloc = new_alloc(100);

        let result = alloc.write_zeros(u64::MAX - 1, 4);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("overflow"));
    }

    #[test]
    fn test_write_zeros() {
        let mut alloc = new_alloc(1024);

        // Write some non-zero data first
        alloc.memory[10] = 42;
        alloc.memory[11] = 43;
        alloc.memory[12] = 44;

        // Zero it out
        alloc.write_zeros(10, 3).unwrap();

        assert_eq!(alloc.memory[10], 0);
        assert_eq!(alloc.memory[11], 0);
        assert_eq!(alloc.memory[12], 0);
    }

    #[test]
    fn test_write_zeros_out_of_bounds() {
        let mut alloc = new_alloc(100);

        let result = alloc.write_zeros(98, 5);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("out of bounds"));
    }

    #[test]
    fn test_read_basic() {
        let mut alloc = new_alloc(1024);

        // Write some data
        alloc.memory[0] = 10;
        alloc.memory[1] = 20;
        alloc.memory[2] = 30;

        // Read it back
        let data = alloc.read(0, 3).unwrap();
        assert_eq!(data, vec![10, 20, 30]);
    }

    #[test]
    fn test_read_out_of_bounds() {
        let alloc = new_alloc(100);

        let result = alloc.read(98, 5);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("out of bounds"));
    }

    #[test]
    fn test_read_overflow() {
        let alloc = new_alloc(100);

        let result = alloc.read(u64::MAX - 1, 5);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("overflow"));
    }

    #[test]
    fn test_read_u8() {
        let mut alloc = new_alloc(1024);
        alloc.memory[10] = 42;

        let value = alloc.read_value::<u8>(10).unwrap();
        assert_eq!(value, 42);
    }

    #[test]
    fn test_read_i8() {
        let mut alloc = new_alloc(1024);
        alloc.memory[10] = 255; // -1 as i8

        let value = alloc.read_value::<i8>(10).unwrap();
        assert_eq!(value, -1);
    }

    #[test]
    fn test_read_u16() {
        let mut alloc = new_alloc(1024);

        // Write 0x1234 in little-endian
        alloc.memory[10] = 0x34;
        alloc.memory[11] = 0x12;

        let value = alloc.read_value::<u16>(10).unwrap();
        assert_eq!(value, 0x1234);
    }

    #[test]
    fn test_read_i16() {
        let mut alloc = new_alloc(1024);

        // Write -1 in little-endian
        alloc.memory[10] = 0xFF;
        alloc.memory[11] = 0xFF;

        let value = alloc.read_value::<i16>(10).unwrap();
        assert_eq!(value, -1);
    }

    #[test]
    fn test_read_u32() {
        let mut alloc = new_alloc(1024);

        // Write 0x12345678 in little-endian
        alloc.memory[10] = 0x78;
        alloc.memory[11] = 0x56;
        alloc.memory[12] = 0x34;
        alloc.memory[13] = 0x12;

        let value = alloc.read_value::<u32>(10).unwrap();
        assert_eq!(value, 0x12345678);
    }

    #[test]
    fn test_read_i32() {
        let mut alloc = new_alloc(1024);

        // Write -1 in little-endian
        alloc.memory[10] = 0xFF;
        alloc.memory[11] = 0xFF;
        alloc.memory[12] = 0xFF;
        alloc.memory[13] = 0xFF;

        let value = alloc.read_value::<i32>(10).unwrap();
        assert_eq!(value, -1);
    }

    #[test]
    fn test_read_u64() {
        let mut alloc = new_alloc(1024);

        // Write 0x123456789ABCDEF0 in little-endian
        alloc.memory[10] = 0xF0;
        alloc.memory[11] = 0xDE;
        alloc.memory[12] = 0xBC;
        alloc.memory[13] = 0x9A;
        alloc.memory[14] = 0x78;
        alloc.memory[15] = 0x56;
        alloc.memory[16] = 0x34;
        alloc.memory[17] = 0x12;

        let value = alloc.read_value::<u64>(10).unwrap();
        assert_eq!(value, 0x123456789ABCDEF0);
    }

    #[test]
    fn test_read_i64() {
        let mut alloc = new_alloc(1024);

        // Write -1 in little-endian
        for i in 10..18 {
            alloc.memory[i] = 0xFF;
        }

        let value = alloc.read_value::<i64>(10).unwrap();
        assert_eq!(value, -1);
    }

    #[test]
    fn test_read_f32() {
        let mut alloc = new_alloc(1024);

        // Write 3.14159 as f32 in little-endian
        let bytes = 3.14159f32.to_le_bytes();
        alloc.memory[10..14].copy_from_slice(&bytes);

        let value = alloc.read_value::<f32>(10).unwrap();
        assert!((value - 3.14159).abs() < 0.0001);
    }

    #[test]
    fn test_read_write_bf16() {
        let mut alloc = new_alloc(1024);

        let addr = alloc.alloc(2, 2).unwrap();
        alloc.write_bf16_from_f32(addr, -1.75).unwrap();

        let value = alloc.read_bf16_as_f32(addr).unwrap();
        assert!((value + 1.75).abs() < 0.01);

        let raw = bf16::from_f32(-1.75).to_bits().to_le_bytes();
        assert_eq!(alloc.memory[addr as usize..addr as usize + 2], raw);
    }

    #[test]
    fn test_alloc_write_read_integration() {
        let mut alloc = new_alloc(1024);

        // Allocate space for a u32
        let addr1 = alloc.alloc(4, 4).unwrap();

        // Write a u32 value
        let value1: u32 = 0xDEADBEEF;
        alloc.write(addr1, &value1.to_le_bytes()).unwrap();

        // Read it back
        let read_value1 = alloc.read_value::<u32>(addr1).unwrap();
        assert_eq!(read_value1, 0xDEADBEEF);

        // Allocate space for a f32
        let addr2 = alloc.alloc(4, 4).unwrap();

        // Write a f32 value
        let value2: f32 = 2.71828;
        alloc.write(addr2, &value2.to_le_bytes()).unwrap();

        // Read it back
        let read_value2 = alloc.read_value::<f32>(addr2).unwrap();
        assert!((read_value2 - 2.71828).abs() < 0.0001);

        // Ensure first value is still intact
        let read_value1_again = alloc.read_value::<u32>(addr1).unwrap();
        assert_eq!(read_value1_again, 0xDEADBEEF);
    }

    #[test]
    fn test_multiple_allocations_different_alignments() {
        let mut alloc = new_alloc(1024);

        let addr1 = alloc.alloc(1, 1).unwrap();
        assert_eq!(addr1, 0);

        let addr2 = alloc.alloc(1, 8).unwrap();
        assert_eq!(addr2, 8); // Should be aligned to 8

        let addr3 = alloc.alloc(1, 1).unwrap();
        assert_eq!(addr3, 9);

        let addr4 = alloc.alloc(1, 16).unwrap();
        assert_eq!(addr4, 16); // Should be aligned to 16

        let addr5 = alloc.alloc(1, 32).unwrap();
        assert_eq!(addr5, 32); // Should be aligned to 32
    }

    #[test]
    fn test_write_read_various_types() {
        let mut alloc = new_alloc(1024);

        // u8
        let addr_u8 = alloc.alloc(1, 1).unwrap();
        alloc.write(addr_u8, &[200u8]).unwrap();
        assert_eq!(alloc.read_value::<u8>(addr_u8).unwrap(), 200);

        // i8
        let addr_i8 = alloc.alloc(1, 1).unwrap();
        alloc.write(addr_i8, &[(-100i8) as u8]).unwrap();
        assert_eq!(alloc.read_value::<i8>(addr_i8).unwrap(), -100);

        // u16
        let addr_u16 = alloc.alloc(2, 2).unwrap();
        alloc.write(addr_u16, &50000u16.to_le_bytes()).unwrap();
        assert_eq!(alloc.read_value::<u16>(addr_u16).unwrap(), 50000);

        // i16
        let addr_i16 = alloc.alloc(2, 2).unwrap();
        alloc.write(addr_i16, &(-25000i16).to_le_bytes()).unwrap();
        assert_eq!(alloc.read_value::<i16>(addr_i16).unwrap(), -25000);

        // u32
        let addr_u32 = alloc.alloc(4, 4).unwrap();
        alloc.write(addr_u32, &3000000000u32.to_le_bytes()).unwrap();
        assert_eq!(alloc.read_value::<u32>(addr_u32).unwrap(), 3000000000);

        // i32
        let addr_i32 = alloc.alloc(4, 4).unwrap();
        alloc.write(addr_i32, &(-2000000000i32).to_le_bytes()).unwrap();
        assert_eq!(alloc.read_value::<i32>(addr_i32).unwrap(), -2000000000);

        // u64
        let addr_u64 = alloc.alloc(8, 8).unwrap();
        alloc.write(addr_u64, &10000000000000000000u64.to_le_bytes()).unwrap();
        assert_eq!(alloc.read_value::<u64>(addr_u64).unwrap(), 10000000000000000000);

        // i64
        let addr_i64 = alloc.alloc(8, 8).unwrap();
        alloc.write(addr_i64, &(-5000000000000000000i64).to_le_bytes()).unwrap();
        assert_eq!(alloc.read_value::<i64>(addr_i64).unwrap(), -5000000000000000000);
    }

    #[test]
    fn test_generate_arange_positive_step() {
        let result = generate_arange(0, 10, 2).unwrap();
        assert_eq!(result, vec![0, 2, 4, 6, 8]);
    }

    #[test]
    fn test_generate_arange_negative_step() {
        let result = generate_arange(10, 0, -2).unwrap();
        assert_eq!(result, vec![10, 8, 6, 4, 2]);
    }

    #[test]
    fn test_generate_arange_single_element() {
        let result = generate_arange(5, 6, 1).unwrap();
        assert_eq!(result, vec![5]);
    }

    #[test]
    fn test_generate_arange_empty() {
        let result = generate_arange(5, 5, 1).unwrap();
        assert_eq!(result, vec![]);
    }

    #[test]
    fn test_generate_arange_zero_step() {
        let result = generate_arange(0, 10, 0);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("cannot be 0"));
    }

    #[test]
    fn test_generate_arange_wrong_direction() {
        // Positive step but start > end
        let result = generate_arange(10, 0, 2);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("wrong sign"));

        // Negative step but start < end
        let result = generate_arange(0, 10, -2);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("wrong sign"));
    }

    #[test]
    fn test_generate_arange_negative_step_wrong_direction() {
        let result = generate_arange(-2, 4, -1);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("wrong sign"));
    }

    // LDS tests - reusing patterns from GlobalAlloc tests

    #[test]
    fn test_lds_alloc_basic() {
        let mut lds = LDS::new(1024);

        // First allocation should start at 0
        let addr = lds.alloc(16, 4).unwrap();
        assert_eq!(addr, 0);
        assert_eq!(lds.next, 16);

        // Second allocation should follow immediately
        let addr2 = lds.alloc(32, 4).unwrap();
        assert_eq!(addr2, 16);
        assert_eq!(lds.next, 48);
    }

    #[test]
    fn test_lds_alloc_minimum_alignment() {
        let mut lds = LDS::new(1024);

        // Even with 1-byte alignment request, LDS enforces 4-byte minimum
        let addr = lds.alloc(1, 1).unwrap();
        assert_eq!(addr, 0);
        assert_eq!(lds.next, 1);

        // Next allocation with 1-byte request should align to 4
        let addr2 = lds.alloc(1, 1).unwrap();
        assert_eq!(addr2, 4);
        assert_eq!(lds.next, 5);
    }

    #[test]
    fn test_lds_alloc_alignment() {
        let mut lds = LDS::new(1024);

        // Allocate 1 byte (will be 4-byte aligned minimum)
        let addr1 = lds.alloc(1, 4).unwrap();
        assert_eq!(addr1, 0);
        assert_eq!(lds.next, 1);

        // Allocate with 8-byte alignment
        let addr2 = lds.alloc(4, 8).unwrap();
        assert_eq!(addr2, 8);
        assert_eq!(lds.next, 12);

        // Allocate with 16-byte alignment
        let addr3 = lds.alloc(8, 16).unwrap();
        assert_eq!(addr3, 16);
        assert_eq!(lds.next, 24);
    }

    #[test]
    fn test_lds_alloc_out_of_memory() {
        let mut lds = LDS::new(100);

        // Allocate most of the space
        lds.alloc(90, 4).unwrap();

        // This should fail - not enough space left
        let result = lds.alloc(20, 4);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("out of memory"));
    }

    #[test]
    fn test_lds_alloc_overflow() {
        let mut lds = LDS::new(100);

        // Allocate something first
        lds.alloc(10, 4).unwrap();

        // Try to allocate a size that would overflow
        let result = lds.alloc(usize::MAX - 5, 4);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("overflow"));
    }

    #[test]
    fn test_lds_write_read_basic() {
        let mut lds = LDS::new(1024);

        let data = vec![1, 2, 3, 4, 5];
        lds.write(0, &data).unwrap();

        // Verify data was written
        assert_eq!(lds.memory[0], 1);
        assert_eq!(lds.memory[1], 2);
        assert_eq!(lds.memory[2], 3);
        assert_eq!(lds.memory[3], 4);
        assert_eq!(lds.memory[4], 5);

        // Read it back
        let read_data = lds.read(0, 5).unwrap();
        assert_eq!(read_data, vec![1, 2, 3, 4, 5]);
    }

    #[test]
    fn test_lds_write_zeros() {
        let mut lds = LDS::new(1024);

        // Write some non-zero data first
        lds.memory[10] = 42;
        lds.memory[11] = 43;
        lds.memory[12] = 44;

        // Zero it out
        lds.write_zeros(10, 3).unwrap();

        assert_eq!(lds.memory[10], 0);
        assert_eq!(lds.memory[11], 0);
        assert_eq!(lds.memory[12], 0);
    }

    #[test]
    fn test_lds_read_typed_values() {
        let mut lds = LDS::new(1024);

        // Test u32
        let addr_u32 = lds.alloc(4, 4).unwrap();
        lds.write(addr_u32, &0xDEADBEEFu32.to_le_bytes()).unwrap();
        assert_eq!(lds.read_value::<u32>(addr_u32).unwrap(), 0xDEADBEEF);

        // Test i32
        let addr_i32 = lds.alloc(4, 4).unwrap();
        lds.write(addr_i32, &(-42i32).to_le_bytes()).unwrap();
        assert_eq!(lds.read_value::<i32>(addr_i32).unwrap(), -42);

        // Test f32
        let addr_f32 = lds.alloc(4, 4).unwrap();
        lds.write(addr_f32, &3.14159f32.to_le_bytes()).unwrap();
        let value = lds.read_value::<f32>(addr_f32).unwrap();
        assert!((value - 3.14159).abs() < 0.0001);
    }

    #[test]
    fn test_lds_read_write_bf16() {
        let mut lds = LDS::new(1024);

        let addr = lds.alloc(2, 4).unwrap();
        lds.write_bf16_from_f32(addr, -1.75).unwrap();

        let value = lds.read_bf16_as_f32(addr).unwrap();
        assert!((value + 1.75).abs() < 0.01);
    }

    #[test]
    fn test_lds_alloc_write_read_integration() {
        let mut lds = LDS::new(1024);

        // Allocate space for multiple values
        let addr1 = lds.alloc(4, 4).unwrap();
        let addr2 = lds.alloc(4, 4).unwrap();
        let addr3 = lds.alloc(8, 8).unwrap();

        // Write different typed values
        lds.write(addr1, &100u32.to_le_bytes()).unwrap();
        lds.write(addr2, &(-200i32).to_le_bytes()).unwrap();
        lds.write(addr3, &123456789u64.to_le_bytes()).unwrap();

        // Read them back
        assert_eq!(lds.read_value::<u32>(addr1).unwrap(), 100);
        assert_eq!(lds.read_value::<i32>(addr2).unwrap(), -200);
        assert_eq!(lds.read_value::<u64>(addr3).unwrap(), 123456789);
    }

    #[test]
    fn test_lds_out_of_bounds() {
        let lds = LDS::new(100);

        // Read out of bounds
        let result = lds.read(98, 5);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("out of bounds"));
    }

    #[test]
    fn test_lds_write_out_of_bounds() {
        let mut lds = LDS::new(100);

        let data = vec![1, 2, 3, 4, 5];
        let result = lds.write(98, &data);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("out of bounds"));
    }

    #[test]
    fn exec_lo_hi_wave32_from_sim() {
        let mut wave = WaveState::new(WaveSize::Wave32, 1, 0).unwrap();
        wave.write_special_b32(SpecialRegister::ExecLo, 0xCAFEBABE);
        wave.write_special_b32(SpecialRegister::ExecHi, 0x1234_5678);
        assert_eq!(wave.read_special_b32(SpecialRegister::ExecLo), 0xCAFEBABE);
        assert_eq!(wave.read_special_b32(SpecialRegister::ExecHi), 0);
        assert_eq!(wave.exec_mask(), 0xCAFEBABE);
    }

    #[test]
    fn exec_lo_hi_wave64_from_sim() {
        let mut wave = WaveState::new(WaveSize::Wave64, 1, 0).unwrap();
        wave.write_special_b32(SpecialRegister::ExecLo, 0x1020_3040);
        wave.write_special_b32(SpecialRegister::ExecHi, 0x5060_7080);
        assert_eq!(wave.read_special_b32(SpecialRegister::ExecLo), 0x1020_3040);
        assert_eq!(wave.read_special_b32(SpecialRegister::ExecHi), 0x5060_7080);
        assert_eq!(wave.exec_mask(), 0x5060_7080_1020_3040);
    }

    #[test]
    fn read_vgpr_f32_applies_modifiers() {
        let def = lookup_common_normalized("v_add_f32").unwrap();
        let inst = DecodedInst {
            name: "v_add_f32".to_string(),
            def,
            line_num: 1,
            operands: vec![DecodedOperand::Vgpr(0)],
        };
        let mut wave = WaveState::new(WaveSize::Wave32, 2, 1).unwrap();
        let mut lds = LDS::new(0);
        let mut alloc = new_alloc(0);
        let ctx = Ctx {
            wave: &mut wave,
            lds: &mut lds,
            global_mem: &mut alloc,
            inst: &inst,
        };

        ctx.wave.write_vgpr(1, 0, (-1.5f32).to_bits());
        let abs_op = DecodedOperand::Abs(Box::new(DecodedOperand::Vgpr(1)));
        let neg_abs_op = DecodedOperand::Negate(Box::new(abs_op.clone()));

        assert_eq!(ctx.read_vgpr::<f32>(&abs_op, 0), 1.5);
        assert_eq!(ctx.read_vgpr::<f32>(&neg_abs_op, 0), -1.5);
    }

    #[test]
    fn read_vgpr_i32_applies_modifiers() {
        let def = lookup_common_normalized("v_add_nc_i32").unwrap();
        let inst = DecodedInst {
            name: "v_add_nc_i32".to_string(),
            def,
            line_num: 1,
            operands: vec![DecodedOperand::Vgpr(0)],
        };
        let mut wave = WaveState::new(WaveSize::Wave32, 2, 1).unwrap();
        let mut lds = LDS::new(0);
        let mut alloc = new_alloc(0);
        let ctx = Ctx {
            wave: &mut wave,
            lds: &mut lds,
            global_mem: &mut alloc,
            inst: &inst,
        };

        ctx.wave.write_vgpr(1, 0, (-7i32) as u32);
        let abs_op = DecodedOperand::Abs(Box::new(DecodedOperand::Vgpr(1)));
        let neg_abs_op = DecodedOperand::Negate(Box::new(abs_op.clone()));

        assert_eq!(ctx.read_vgpr::<i32>(&abs_op, 0), 7);
        assert_eq!(ctx.read_vgpr::<i32>(&neg_abs_op, 0), -7);
    }

    #[test]
    fn read_vgpr_bf16_as_f32_applies_modifiers() {
        let def = lookup_common_normalized("v_add_f32").unwrap();
        let inst = DecodedInst {
            name: "v_add_f32".to_string(),
            def,
            line_num: 1,
            operands: vec![DecodedOperand::Vgpr(0)],
        };
        let mut wave = WaveState::new(WaveSize::Wave32, 2, 1).unwrap();
        let mut lds = LDS::new(0);
        let mut alloc = new_alloc(0);
        let ctx = Ctx {
            wave: &mut wave,
            lds: &mut lds,
            global_mem: &mut alloc,
            inst: &inst,
        };

        let bits = bf16::from_f32(-2.0).to_bits() as u32;
        ctx.wave.write_vgpr(1, 0, bits);
        let abs_op = DecodedOperand::Abs(Box::new(DecodedOperand::Vgpr(1)));
        let neg_abs_op = DecodedOperand::Negate(Box::new(abs_op.clone()));

        assert_eq!(ctx.read_vgpr_bf16_as_f32(&abs_op, 0), 2.0);
        assert_eq!(ctx.read_vgpr_bf16_as_f32(&neg_abs_op, 0), -2.0);
    }

    #[test]
    fn write_vgpr_bf16_from_f32_roundtrip() {
        let def = lookup_common_normalized("v_add_f32").unwrap();
        let inst = DecodedInst {
            name: "v_add_f32".to_string(),
            def,
            line_num: 1,
            operands: vec![DecodedOperand::Vgpr(0)],
        };
        let mut wave = WaveState::new(WaveSize::Wave32, 2, 1).unwrap();
        let mut lds = LDS::new(0);
        let mut alloc = new_alloc(0);
        let mut ctx = Ctx {
            wave: &mut wave,
            lds: &mut lds,
            global_mem: &mut alloc,
            inst: &inst,
        };

        ctx.write_vgpr_bf16_from_f32(1, 0, 3.5);
        let bits = ctx.wave.read_vgpr(1, 0) as u16;
        assert_eq!(bits, bf16::from_f32(3.5).to_bits());
    }
}
