pub mod isa;
pub mod ops;
mod parse;
mod sim;
pub mod wave;

use std::{path::PathBuf, str::FromStr};

use crate::sim::GlobalAlloc;

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
    #[value(name = "32", alias = "wave32", alias = "wave-32")]
    Wave32,
    #[value(name = "64", alias = "wave64", alias = "wave-64")]
    Wave64,
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

// includes state for the whole program
pub struct Program {
    pub global_mem: GlobalAlloc,
    pub local_launch_size: Dim3,
    pub global_launch_size: Dim3,
    // ? do we want to simulate traps? probably not
    // TBA - trap base address
    // TMA - trap memory address
}

impl Program {
    pub fn new(global_mem_size: usize) -> Self {
        Self {
            global_mem: GlobalAlloc::new(global_mem_size),
            local_launch_size: Dim3::new(1, 1, 1),
            global_launch_size: Dim3::new(1, 1, 1),
        }
    }

    pub fn alloc_global(&mut self, size: usize, align: usize) -> Result<u64, String> {
        self.global_mem.alloc(size, align)
    }

    pub fn write_global(&mut self, addr: u64, data: &[u8]) -> Result<(), String> {
        self.global_mem.write(addr, data)
    }

    pub fn write_global_zeros(&mut self, addr: u64, size: usize) -> Result<(), String> {
        self.global_mem.write_zeros(addr, size)
    }

    pub fn read_global(&self, addr: u64, size: usize) -> Result<Vec<u8>, String> {
        self.global_mem.read(addr, size)
    }
}

pub fn run_file(
    file: Option<PathBuf>,
    _arch: Architecture,
    wave_size: WaveSize,
    global_mem_size: usize,
    _debug: bool,
) -> Result<(), String> {
    if let Some(file_path) = file {
        let mut program = Program::new(global_mem_size);
        let program_info = parse::parse_file(&file_path, &mut program)?;
        program.local_launch_size = program_info.local_launch_size;
        program.global_launch_size = program_info.global_launch_size;
        let wave_size = program_info.wave_size.unwrap_or(wave_size);

        println!("Parsed arguments:");
        for arg in &program_info.arguments {
            println!("  {} : {} @ 0x{:x}", arg.name, arg.type_name, arg.addr);
        }

        println!("Parsed output arguments:");
        for arg in &program_info.output_arguments {
            println!("  {} : {} @ 0x{:x}", arg.name, arg.type_name, arg.addr);
        }

        println!("Local launch size: {:?}", program.local_launch_size);
        println!("Global launch size: {:?}", program.global_launch_size);
        println!("Wave size: {:?}", wave_size);
    }
    // the whole parsing flow happens here

    Ok(())
}

// add debug run here, invoking repl and other things
