// Wave state and register tracking live here.
use std::ops::{Deref, DerefMut};

use crate::WaveSize;

// RDNA3.5 specific numbers for register counts, etc.
pub const SGPR_COUNT: usize = 128; // user-accessible 106; beyond this is specials and trap handler registers.
pub const VGPR_MAX: usize = 256; // user chooses this when running kernel.
const VCC_HI: usize = 106;
const VCC_LO: usize = 107;

pub struct SGPRs {
  sgprs: Box<[u32]>,
}

impl Deref for SGPRs {
  type Target = [u32];
  fn deref(&self) -> &Self::Target {
    &self.sgprs[..]
  }
}

impl DerefMut for SGPRs {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.sgprs[..]
  }
}

impl SGPRs {
  fn new() -> Self {
    Self {
      sgprs: Box::new([0; SGPR_COUNT]),
    }
  }

  fn get(&self, idx: usize) -> Option<u32> {
    self.sgprs.get(idx).copied()
  }

  fn set(&mut self, idx: usize, v: u32) -> bool {
    let Some(slot) = self.sgprs.get_mut(idx) else {
      return false;
    };
    *slot = v;
    true
  }

  fn read_vcc_lo(&self) -> u32 {
    self.get(VCC_LO).unwrap_or(0)
  }

  fn read_vcc_hi(&self) -> u32 {
    self.get(VCC_HI).unwrap_or(0)
  }

  fn write_vcc_lo(&mut self, v: u32) {
    let _ = self.set(VCC_LO, v);
  }

  fn write_vcc_hi(&mut self, v: u32) {
    let _ = self.set(VCC_HI, v);
  }

  fn read_u64_pair(&self, hi: usize, lo: usize) -> u64 {
    ((self.get(hi).unwrap_or(0) as u64) << 32) | (self.get(lo).unwrap_or(0) as u64)
  }

  fn write_u64_pair(&mut self, hi: usize, lo: usize, v: u64) {
    let _ = self.set(hi, (v >> 32) as u32);
    let _ = self.set(lo, v as u32);
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
  lanes: usize,
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
    let lanes = if wave == WaveSize::Wave32 { 32 } else { 64 };
    Ok(Self {
      vgpr_file: vec![0; vgprs_rounded * lanes].into_boxed_slice(),
      lanes,
    })
  }

  fn get(&self, vgpr: usize, lane: usize) -> Option<u32> {
    if lane >= self.lanes {
      return None;
    }
    let idx = vgpr * self.lanes + lane;
    self.vgpr_file.get(idx).copied()
  }

  fn set(&mut self, vgpr: usize, lane: usize, v: u32) -> bool {
    if lane >= self.lanes {
      return false;
    }
    let idx = vgpr * self.lanes + lane;
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
  sgprs: SGPRs, // scalar general purpose registers
  // user chooses this when launching kernel
  vgprs: VGPRs, // u32 per lane; allocation rounds up to blocks of 16 (wave32) or 8 (wave64)
  lds: [u32; 128], // 64 kB, change later
  exec: u64,    // only the bottom half is used in wave32
  execz: bool,  // is exec zero
  vcc: u64,     // vector condition code
  vccz: bool,   // is vcc zero
  scc: bool,
  flat_scratch: u64, // technically 48-bit, ignore top 16 bits
  m0: u32,
  // not simulating trap registers, otherwise they'd be here
  vmcnt: u8,   // actually 6 bit
  vscnt: u8,   // also 6 bit
  lgkmcnt: u8, // also 6 bit
}
