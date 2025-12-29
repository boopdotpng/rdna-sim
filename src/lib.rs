mod ir;
pub mod isa;
mod parse;
mod program;
mod sim;
mod wave;

// RDNA3.5 specific numbers for registers, etc 
pub const SGPR_COUNT: usize = 128; // user-accessible; beyond this is specials and trap handler registers
pub const VGPR_COUNT: usize = 255;
pub const LANES: usize = 32;

use std::{path::PathBuf, str::FromStr};

use clap::ValueEnum;

#[derive(Copy, Clone, Debug, Eq, PartialEq, ValueEnum)]
pub enum Architecture {
  // only support 3.5 right now
  // Rdna1,
  // Rdna2,
  // Rdna3,
  #[value(name = "rdna3.5", alias = "rdna3-5", alias = "rdna3_5")]
  Rdna35,
  // Rdna4,
  // Cdna1,
  // Cdna2,
  // Cdna3,
  // Cdna4,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, ValueEnum)]
pub enum WaveSize {
  // only wave32 support for now
  #[value(name = "32", alias = "wave32", alias = "wave-32")]
  Wave32,
  // #[value(name = "64", alias = "wave64", alias = "wave-64")]
  // Wave64,
}

// arguments are stored in global memory, then a pointer to those is stored in 2 SGPRs 
// this is for threads, blocks, and grid size
#[derive(Clone, Debug)]
pub struct Dim3(pub u32, pub u32, pub u32);
impl Dim3 {
  pub const fn new(x: u32, y: u32, z: u32) -> Self {
    Self(x,y,z)
  }
}

impl FromStr for Dim3 {
  type Err = String;
  
  fn from_str(s: &str) -> Result<Self, Self::Err> {
    let mut it = s
      .split(|c| c == ',' || c == 'x' || c == 'X')
      .filter(|p| !p.is_empty())
      .map(|p| p.parse::<u32>().map_err(|e| e.to_string()));

    let x = it.next().ok_or("expected 3 values like 8,8,1 for dim3")??;
    let y = it.next().ok_or("expected 3 values like 8,8,1 for dim3")??;
    let z = it.next().ok_or("expected 3 values like 8,8,1 for dim3")??;

    Ok(Dim3(x,y,z))
  }
}

// registers and memory structures 
pub struct Reg32 {}


// includes state for the whole program
pub struct Program {
  // instruction list

  // global memory (512mb to start) 
  // ? do we want to simulate traps? probably not
  // TBA - trap base address
  // TMA - trap memory address 
}

pub struct WaveState {
  pc: u64,
  sgprs: [u32]
}

pub fn run_file(
  file: Option<PathBuf>,
  arch: Architecture,
  wave_size: WaveSize,
  debug: bool,
) -> Result<(), String> {
  let _ = (file, arch, wave_size, debug);
  // the whole parsing flow happens here 




  Ok(())
}

// add debug run here, invoking repl and other things 