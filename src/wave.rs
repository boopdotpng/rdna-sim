// Wave state and register tracking live here.
use crate::WaveSize;


// RDNA3.5 specific numbers for register counts, etc.
pub const SGPR_COUNT: usize = 128; // user-accessible 106; beyond this is specials and trap handler registers.
pub const VGPR_MAX: usize = 256; // user chooses this when running kernel.
const VCC_LO: usize = 106; // s[106:107] with s106=low, s107=high

pub struct SGPRs {
  sgprs: Box<[u32]>,
}

impl SGPRs {
  fn new() -> Self {
    Self {
      sgprs: Box::new([0; SGPR_COUNT]),
    }
  }

  pub fn get(&self, idx: usize) -> Option<u32> {
    self.sgprs.get(idx).copied()
  }

  pub fn set(&mut self, idx: usize, v: u32) -> bool {
    let Some(slot) = self.sgprs.get_mut(idx) else {
      return false;
    };
    *slot = v;
    true
  }

  /// Reads a 64-bit value from consecutive SGPRs [lo, lo+1]
  pub fn read_pair(&self, lo: usize) -> u64 {
    let lo_val = self.get(lo).unwrap_or(0) as u64;
    let hi_val = self.get(lo + 1).unwrap_or(0) as u64;
    (hi_val << 32) | lo_val
  }

  /// Writes a 64-bit value to consecutive SGPRs [lo, lo+1]
  pub fn write_pair(&mut self, lo: usize, v: u64) {
    let _ = self.set(lo, v as u32);
    let _ = self.set(lo + 1, (v >> 32) as u32);
  }

  /// Reads a 128-bit value from consecutive SGPRs [lo, lo+1, lo+2, lo+3]
  pub fn read_quad(&self, lo: usize) -> u128 {
    let v0 = self.get(lo).unwrap_or(0) as u128;
    let v1 = self.get(lo + 1).unwrap_or(0) as u128;
    let v2 = self.get(lo + 2).unwrap_or(0) as u128;
    let v3 = self.get(lo + 3).unwrap_or(0) as u128;
    (v3 << 96) | (v2 << 64) | (v1 << 32) | v0
  }

  /// Writes a 128-bit value to consecutive SGPRs [lo, lo+1, lo+2, lo+3]
  pub fn write_quad(&mut self, lo: usize, v: u128) {
    let _ = self.set(lo, v as u32);
    let _ = self.set(lo + 1, (v >> 32) as u32);
    let _ = self.set(lo + 2, (v >> 64) as u32);
    let _ = self.set(lo + 3, (v >> 96) as u32);
  }
}

/*
Each thread in the wave gets a u32.
v0 is usually threadIdx.x (work-item id).
v1 is threadIdx.y
at least from kernels i've read
*/
pub struct VGPRs {
  vgpr_file: Box<[u32]>,
  threads: usize,
}

impl VGPRs {
  fn new(wave: WaveSize, vgpr_count: usize) -> Result<Self, String> {
    if vgpr_count == 0 {
      return Err("wave must allocate at least one VGPR".to_string());
    }
    if vgpr_count > VGPR_MAX {
      return Err(format!(
        "vgpr_count {} exceeds VGPR_MAX {}",
        vgpr_count, VGPR_MAX
      ));
    }
    let block = if wave == WaveSize::Wave32 { 16 } else { 8 };
    let vgprs_rounded = ((vgpr_count + block - 1) / block) * block;
    let threads = if wave == WaveSize::Wave32 { 32 } else { 64 };
    Ok(Self {
      vgpr_file: vec![0; vgprs_rounded * threads].into_boxed_slice(),
      threads,
    })
  }

  pub fn get(&self, vgpr: usize, thread: usize) -> Option<u32> {
    if thread >= self.threads {
      return None;
    }
    let idx = vgpr * self.threads + thread;
    self.vgpr_file.get(idx).copied()
  }

  pub fn set(&mut self, vgpr: usize, thread: usize, v: u32) -> bool {
    if thread >= self.threads {
      return false;
    }
    let idx = vgpr * self.threads + thread;
    let Some(slot) = self.vgpr_file.get_mut(idx) else {
      return false;
    };
    *slot = v;
    true
  }
}

// every wave gets a copy of this
pub struct WaveState {
  pc: u64,      // program counter
  wave_size: WaveSize,
  sgprs: SGPRs, // scalar general purpose registers (VCC is aliased to s[106:107])
  // user chooses this when launching kernel
  vgprs: VGPRs, // u32 per thread; allocation rounds up to blocks of 16 (wave32) or 8 (wave64)
  lds: [u32; 128], // 64 kB, change later
  exec: u64,    // only the bottom 32 bits are used in wave32
  scc: bool,
  flat_scratch: u64, // technically 48-bit, ignore top 16 bits
  m0: u32,
  // not simulating trap registers, otherwise they'd be here
  vmcnt: u8,   // actually 6 bit
  vscnt: u8,   // also 6 bit
  lgkmcnt: u8, // also 6 bit
}

impl WaveState {
  /// Read VCC as a 64-bit value (aliased to s[106:107])
  pub fn read_vcc(&self) -> u64 {
    self.sgprs.read_pair(VCC_LO)
  }

  /// Write VCC as a 64-bit value (aliased to s[106:107])
  pub fn write_vcc(&mut self, v: u64) {
    self.sgprs.write_pair(VCC_LO, v);
  }

  /// Check if VCC is zero
  pub fn vccz(&self) -> bool {
    self.read_vcc() == 0
  }

  /// Check if EXEC is zero (wave32 checks bottom 32 bits, wave64 checks all 64 bits)
  pub fn execz(&self) -> bool {
    match self.wave_size {
      WaveSize::Wave32 => (self.exec & 0xFFFFFFFF) == 0,
      WaveSize::Wave64 => self.exec == 0,
    }
  }

  /// Increment PC by a given offset (typically 4 bytes per instruction)
  pub fn increment_pc(&mut self, offset: u64) {
    self.pc = self.pc.wrapping_add(offset);
  }

  /// Jump PC to an absolute address
  pub fn jump_to(&mut self, addr: u64) {
    self.pc = addr;
  }

  /// Get current PC value
  pub fn pc(&self) -> u64 {
    self.pc
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn sgprs_get_set_bounds() {
    let mut sgprs = SGPRs::new();
    assert_eq!(sgprs.get(0), Some(0));
    assert!(sgprs.set(0, 7));
    assert_eq!(sgprs.get(0), Some(7));
    assert_eq!(sgprs.get(SGPR_COUNT), None);
    assert!(!sgprs.set(SGPR_COUNT, 1));
  }

  #[test]
  fn sgprs_pair_and_quad_read_write() {
    let mut sgprs = SGPRs::new();
    let v64 = 0x11223344_55667788u64;
    sgprs.write_pair(2, v64);
    assert_eq!(sgprs.read_pair(2), v64);
    assert_eq!(sgprs.get(2), Some(0x55667788));
    assert_eq!(sgprs.get(3), Some(0x11223344));

    let v128 = 0x01020304_05060708_11223344_55667788u128;
    sgprs.write_quad(4, v128);
    assert_eq!(sgprs.read_quad(4), v128);

    sgprs.set(SGPR_COUNT - 1, 0xAABBCCDD);
    assert_eq!(sgprs.read_pair(SGPR_COUNT - 1), 0xAABBCCDDu64);
    assert_eq!(
      sgprs.read_quad(SGPR_COUNT - 2),
      (0xAABBCCDDu128) << 32
    );
  }

  #[test]
  fn vgprs_new_validation() {
    assert!(VGPRs::new(WaveSize::Wave32, 0).is_err());
    assert!(VGPRs::new(WaveSize::Wave64, VGPR_MAX + 1).is_err());
  }

  #[test]
  fn vgprs_thread_bounds() {
    let mut vgprs = VGPRs::new(WaveSize::Wave32, 1).unwrap();
    assert!(vgprs.set(0, 31, 99));
    assert!(!vgprs.set(0, 32, 99));
    assert_eq!(vgprs.get(0, 31), Some(99));
    assert_eq!(vgprs.get(0, 32), None);

    let mut vgprs = VGPRs::new(WaveSize::Wave64, 1).unwrap();
    assert!(vgprs.set(0, 63, 42));
    assert!(!vgprs.set(0, 64, 42));
    assert_eq!(vgprs.get(0, 63), Some(42));
    assert_eq!(vgprs.get(0, 64), None);
  }

  #[test]
  fn vgprs_rounding_and_indexing() {
    let mut vgprs = VGPRs::new(WaveSize::Wave32, 17).unwrap();
    assert!(vgprs.set(31, 0, 7));
    assert!(!vgprs.set(32, 0, 7));
    assert_eq!(vgprs.get(31, 0), Some(7));
    assert_eq!(vgprs.get(32, 0), None);
  }

  fn new_wave_state(wave_size: WaveSize, exec: u64) -> WaveState {
    WaveState {
      pc: 0,
      wave_size,
      sgprs: SGPRs::new(),
      vgprs: VGPRs::new(wave_size, 1).unwrap(),
      lds: [0; 128],
      exec,
      scc: false,
      flat_scratch: 0,
      m0: 0,
      vmcnt: 0,
      vscnt: 0,
      lgkmcnt: 0,
    }
  }

  #[test]
  fn wave_execz_wave32_wave64() {
    let wave32 = new_wave_state(WaveSize::Wave32, 0xFFFF_FFFF_0000_0000);
    assert!(wave32.execz());
    let wave32 = new_wave_state(WaveSize::Wave32, 0x0000_0000_0000_0001);
    assert!(!wave32.execz());

    let wave64 = new_wave_state(WaveSize::Wave64, 0xFFFF_FFFF_0000_0000);
    assert!(!wave64.execz());
    let wave64 = new_wave_state(WaveSize::Wave64, 0);
    assert!(wave64.execz());
  }
}
