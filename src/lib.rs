mod ir;
pub mod isa;
mod parse;
mod program;
mod sim;
mod wave;
use std::ops::{Deref, DerefMut};

// RDNA3.5 specific numbers for register counts, etc
pub const SGPR_COUNT: usize = 128; // user-accessible 106; beyond this is specials and trap handler registers which we don't care about at the moment 
pub const VGPR_MAX: usize = 256; // user chooses this when running kernel
const VCC_HI: usize = 106;
const VCC_LO: usize = 107;

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
        Self(x, y, z)
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

        Ok(Dim3(x, y, z))
    }
}

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

// includes state for the whole program
pub struct Program {
    // instruction list with arguments

    // global memory (64MB to start, flat)
    global_mem: Box<[u64]>,
    // launch sizes for the program
    pub local_launch_size: Dim3,
    pub global_launch_size: Dim3,
    // ? do we want to simulate traps? probably not
    // TBA - trap base address
    // TMA - trap memory address
}

// Stub for global memory allocator - will be implemented later
pub struct GlobalAlloc {
    // Placeholder for memory allocator implementation
    _placeholder: u64,
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

pub fn run_file(
    file: Option<PathBuf>,
    arch: Architecture,
    wave_size: WaveSize,
    _debug: bool,
) -> Result<(), String> {
    if let Some(file_path) = file {
        // Parse the file using the new parsing functions
        let program_info = parse::parse_top(&file_path)?;
        
        // Print parsed arguments for verification
        println!("Parsed arguments:");
        for (name, type_str) in &program_info.arguments {
            println!("  {} : {}", name, type_str);
        }
        
        println!("Parsed output arguments:");
        for (name, type_str) in &program_info.output_arguments {
            println!("  {} : {}", name, type_str);
        }
        
        println!("Parsed print addresses:");
        for (addr, type_str) in &program_info.print_addresses {
            println!("  0x{:x} : {}", addr, type_str);
        }
        
        println!("Local launch size: {:?}", program_info.local_launch_size);
        println!("Global launch size: {:?}", program_info.global_launch_size);
        
        let program = parse::parse_into_program(program_info, arch, wave_size);
        
        // Here we would typically run the simulation with the launch sizes
        // For now, we'll just print them to show they're properly initialized
        println!("Program created with local_launch_size: {:?} and global_launch_size: {:?}", 
                 program.local_launch_size, program.global_launch_size);
    }
    // the whole parsing flow happens here

    Ok(())
}

// add debug run here, invoking repl and other things
