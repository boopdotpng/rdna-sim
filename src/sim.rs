use crate::isa::InstructionCommonDef;
use crate::wave::WaveState;
use crate::Program;

#[derive(Clone, Debug)]
pub struct GlobalAlloc {
    pub(crate) memory: Box<[u8]>, // not dynamic. u8 is divisible by all byte-widths used, f32, f16, etc. 
    pub(crate) next: usize, // next available address for allocation
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

  pub fn write(&mut self, addr: u64, data: &[u8]) -> Result<(), String> {
    let start = addr as usize;
    let end = start
      .checked_add(data.len())
      .ok_or_else(|| "global write overflow".to_string())?;
    if end > self.memory.len() {
      return Err(format!(
        "global write out of bounds: {}..{} (len {})",
        start,
        end,
        self.memory.len()
      ));
    }
    self.memory[start..end].copy_from_slice(data);
    Ok(())
  }

  pub fn write_zeros(&mut self, addr: u64, size: usize) -> Result<(), String> {
    let start = addr as usize;
    let end = start
      .checked_add(size)
      .ok_or_else(|| "global write overflow".to_string())?;
    if end > self.memory.len() {
      return Err(format!(
        "global write out of bounds: {}..{} (len {})",
        start,
        end,
        self.memory.len()
      ));
    }
    self.memory[start..end].fill(0);
    Ok(())
  }

  pub fn read(&self, addr: u64, size: usize) -> Result<Vec<u8>, String> {
    let start = addr as usize;
    let end = start
      .checked_add(size)
      .ok_or_else(|| "global read overflow".to_string())?;
    if end > self.memory.len() {
      return Err(format!(
        "global read out of bounds: {}..{} (len {})",
        start,
        end,
        self.memory.len()
      ));
    }
    Ok(self.memory[start..end].to_vec())
  }

  pub fn read_u8(&self, addr: u64) -> Result<u8, String> {
    Ok(self.read(addr, 1)?[0])
  }

  pub fn read_i8(&self, addr: u64) -> Result<i8, String> {
    Ok(self.read_u8(addr)? as i8)
  }

  pub fn read_u16(&self, addr: u64) -> Result<u16, String> {
    let bytes = self.read(addr, 2)?;
    Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
  }

  pub fn read_i16(&self, addr: u64) -> Result<i16, String> {
    Ok(self.read_u16(addr)? as i16)
  }

  pub fn read_u32(&self, addr: u64) -> Result<u32, String> {
    let bytes = self.read(addr, 4)?;
    Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
  }

  pub fn read_i32(&self, addr: u64) -> Result<i32, String> {
    Ok(self.read_u32(addr)? as i32)
  }

  pub fn read_u64(&self, addr: u64) -> Result<u64, String> {
    let bytes = self.read(addr, 8)?;
    Ok(u64::from_le_bytes([
      bytes[0], bytes[1], bytes[2], bytes[3],
      bytes[4], bytes[5], bytes[6], bytes[7],
    ]))
  }

  pub fn read_i64(&self, addr: u64) -> Result<i64, String> {
    Ok(self.read_u64(addr)? as i64)
  }

  pub fn read_f32(&self, addr: u64) -> Result<f32, String> {
    let bytes = self.read(addr, 4)?;
    Ok(f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
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

#[cfg(test)]
mod tests {
    use super::*;

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

        let value = alloc.read_u8(10).unwrap();
        assert_eq!(value, 42);
    }

    #[test]
    fn test_read_i8() {
        let mut alloc = new_alloc(1024);
        alloc.memory[10] = 255; // -1 as i8

        let value = alloc.read_i8(10).unwrap();
        assert_eq!(value, -1);
    }

    #[test]
    fn test_read_u16() {
        let mut alloc = new_alloc(1024);

        // Write 0x1234 in little-endian
        alloc.memory[10] = 0x34;
        alloc.memory[11] = 0x12;

        let value = alloc.read_u16(10).unwrap();
        assert_eq!(value, 0x1234);
    }

    #[test]
    fn test_read_i16() {
        let mut alloc = new_alloc(1024);

        // Write -1 in little-endian
        alloc.memory[10] = 0xFF;
        alloc.memory[11] = 0xFF;

        let value = alloc.read_i16(10).unwrap();
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

        let value = alloc.read_u32(10).unwrap();
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

        let value = alloc.read_i32(10).unwrap();
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

        let value = alloc.read_u64(10).unwrap();
        assert_eq!(value, 0x123456789ABCDEF0);
    }

    #[test]
    fn test_read_i64() {
        let mut alloc = new_alloc(1024);

        // Write -1 in little-endian
        for i in 10..18 {
            alloc.memory[i] = 0xFF;
        }

        let value = alloc.read_i64(10).unwrap();
        assert_eq!(value, -1);
    }

    #[test]
    fn test_read_f32() {
        let mut alloc = new_alloc(1024);

        // Write 3.14159 as f32 in little-endian
        let bytes = 3.14159f32.to_le_bytes();
        alloc.memory[10..14].copy_from_slice(&bytes);

        let value = alloc.read_f32(10).unwrap();
        assert!((value - 3.14159).abs() < 0.0001);
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
        let read_value1 = alloc.read_u32(addr1).unwrap();
        assert_eq!(read_value1, 0xDEADBEEF);

        // Allocate space for a f32
        let addr2 = alloc.alloc(4, 4).unwrap();

        // Write a f32 value
        let value2: f32 = 2.71828;
        alloc.write(addr2, &value2.to_le_bytes()).unwrap();

        // Read it back
        let read_value2 = alloc.read_f32(addr2).unwrap();
        assert!((read_value2 - 2.71828).abs() < 0.0001);

        // Ensure first value is still intact
        let read_value1_again = alloc.read_u32(addr1).unwrap();
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
        assert_eq!(alloc.read_u8(addr_u8).unwrap(), 200);

        // i8
        let addr_i8 = alloc.alloc(1, 1).unwrap();
        alloc.write(addr_i8, &[(-100i8) as u8]).unwrap();
        assert_eq!(alloc.read_i8(addr_i8).unwrap(), -100);

        // u16
        let addr_u16 = alloc.alloc(2, 2).unwrap();
        alloc.write(addr_u16, &50000u16.to_le_bytes()).unwrap();
        assert_eq!(alloc.read_u16(addr_u16).unwrap(), 50000);

        // i16
        let addr_i16 = alloc.alloc(2, 2).unwrap();
        alloc.write(addr_i16, &(-25000i16).to_le_bytes()).unwrap();
        assert_eq!(alloc.read_i16(addr_i16).unwrap(), -25000);

        // u32
        let addr_u32 = alloc.alloc(4, 4).unwrap();
        alloc.write(addr_u32, &3000000000u32.to_le_bytes()).unwrap();
        assert_eq!(alloc.read_u32(addr_u32).unwrap(), 3000000000);

        // i32
        let addr_i32 = alloc.alloc(4, 4).unwrap();
        alloc.write(addr_i32, &(-2000000000i32).to_le_bytes()).unwrap();
        assert_eq!(alloc.read_i32(addr_i32).unwrap(), -2000000000);

        // u64
        let addr_u64 = alloc.alloc(8, 8).unwrap();
        alloc.write(addr_u64, &10000000000000000000u64.to_le_bytes()).unwrap();
        assert_eq!(alloc.read_u64(addr_u64).unwrap(), 10000000000000000000);

        // i64
        let addr_i64 = alloc.alloc(8, 8).unwrap();
        alloc.write(addr_i64, &(-5000000000000000000i64).to_le_bytes()).unwrap();
        assert_eq!(alloc.read_i64(addr_i64).unwrap(), -5000000000000000000);
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
}