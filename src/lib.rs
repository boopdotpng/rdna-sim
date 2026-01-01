pub mod isa;
pub mod ops;
mod decode;
mod parse;
pub mod parse_instruction;
mod scheduler;
mod sim;
pub mod wave;

use std::{path::PathBuf, str::FromStr};

use crate::sim::{GlobalAlloc, MemoryOps};
use half::bf16;

use clap::ValueEnum;

#[derive(Copy, Clone, Debug, Eq, PartialEq, ValueEnum)]
pub enum Architecture {
    // only support 3.5 right now
    // Rdna1,
    // Rdna2,
    // Rdna3,
    // rdna 3.5 will get support first. then rdna4. i don't have an rdna3 card
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
#[derive(Clone, Debug, PartialEq)]
pub struct Dim3(pub u32, pub u32, pub u32);
impl Dim3 {
    pub const fn new(x: u32, y: u32, z: u32) -> Self { Self(x, y, z) } 

    pub fn linear_len(&self) -> u64 {
        self.0 as u64 * self.1 as u64 * self.2 as u64
    }

    pub fn split_linear(&self, linear: u64) -> (u32, u32, u32) {
        let x = (linear % self.0 as u64) as u32;
        let y = ((linear / self.0 as u64) % self.1 as u64) as u32;
        let z = (linear / (self.0 as u64 * self.1 as u64)) as u32;
        (x, y, z)
    }
}

impl FromStr for Dim3 {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim();
        let s = s.strip_prefix('(').unwrap_or(s);
        let s = s.strip_suffix(')').unwrap_or(s);

        let mut it = s
            .split(',')
            .map(|p| p.trim())
            .filter(|p| !p.is_empty())
            .map(|p| p.parse::<u32>().map_err(|e| e.to_string()));

        let x = it.next().ok_or("expected 3 values like 1,1,1 for dim3")??;
        let y = it.next().ok_or("expected 3 values like 1,1,1 for dim3")??;
        let z = it.next().ok_or("expected 3 values like 1,1,1 for dim3")??;

        Ok(Dim3(x, y, z))
    }
}

// state for the entire program 
pub struct Program {
    pub global_mem: GlobalAlloc, // simple bump allocator for simulated global memory
    pub local_launch_size: Dim3, // blocks 
    pub global_launch_size: Dim3, // grids
    pub wave_size: WaveSize,
    // ? do we want to simulate traps? probably not
    // TBA - trap base address
    // TMA - trap memory address
}

impl Program {
    pub fn new(
        global_mem_size: usize,
        local_launch_size: Dim3,
        global_launch_size: Dim3,
        wave_size: WaveSize,
    ) -> Self {
        Self {
            global_mem: GlobalAlloc {
                memory: vec![0; global_mem_size].into_boxed_slice(),
                next: 0,
            },
            local_launch_size,
            global_launch_size,
            wave_size,
        }
    }

    // these methods need to be here -- program owns global allocation
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

    // max 256 threads per workgroup for now. 1024 can be trivially supported but that's usually not a good idea 
    pub fn validate_launch_config(&self) -> Result<(), String> {
        let local = &self.local_launch_size;
        let global = &self.global_launch_size;

        if local.0 == 0 || local.1 == 0 || local.2 == 0 {
            return Err(format!(
                "local launch size cannot have zero dimensions: ({}, {}, {})",
                local.0, local.1, local.2
            ));
        }

        if global.0 == 0 || global.1 == 0 || global.2 == 0 {
            return Err(format!(
                "global launch size cannot have zero dimensions: ({}, {}, {})",
                global.0, global.1, global.2
            ));
        }

        if global.0 < local.0 {
            return Err(format!(
                "global size x ({}) must be >= local size x ({})",
                global.0, local.0
            ));
        }

        if global.1 < local.1 {
            return Err(format!(
                "global size y ({}) must be >= local size y ({})",
                global.1, local.1
            ));
        }

        if global.2 < local.2 {
            return Err(format!(
                "global size z ({}) must be >= local size z ({})",
                global.2, local.2
            ));
        }

        let total_threads = local.0 as u64 * local.1 as u64 * local.2 as u64;
        if total_threads > 256 {
            return Err(format!(
                "local launch size ({}, {}, {}) produces {} threads per workgroup (max 256 for RDNA 3.5)",
                local.0, local.1, local.2, total_threads
            ));
        }

        Ok(())
    }
}

pub fn run_file(
    file: Option<PathBuf>,
    arch: Architecture,
    wave_size: WaveSize,
    global_mem_size: usize,
    _debug: bool,
) -> Result<(), String> {
    if let Some(file_path) = file {
        let mut program = Program::new(
            global_mem_size,
            Dim3::new(64, 1, 1),
            Dim3::new(64, 1, 1),
            wave_size,
        );
        let program_info = parse::parse_file(&file_path, &mut program, arch)?;
        program.local_launch_size = program_info.local_launch_size.clone();
        program.global_launch_size = program_info.global_launch_size.clone();
        program.wave_size = program_info.wave_size.unwrap_or(program.wave_size);

        program.validate_launch_config()?;

        println!("non-output arguments:");
        for arg in &program_info.arguments {
            println!("  {} : {} @ 0x{:x} in global mem", arg.name, arg.type_name, arg.addr);
        }

        println!("output arguments:");
        for arg in &program_info.output_arguments {
            println!("  {} : {} @ 0x{:x} in global mem", arg.name, arg.type_name, arg.addr);
        }

        println!("local: {:?}", program.local_launch_size);
        println!("global: {:?}", program.global_launch_size);
        println!("wave: {:?}", program.wave_size);
        scheduler::run_program(&mut program, &program_info, arch)?;

        println!("output values:");
        for arg in &program_info.output_arguments {
            let values = read_output_arg(&program, arg)?;
            if values.len() == 1 {
                println!("  {} = {}", arg.name, values[0]);
            } else {
                println!("  {} = {:?}", arg.name, values);
            }
        }
    }
    Ok(())
}

fn read_output_arg(program: &Program, arg: &parse::ArgInfo) -> Result<Vec<String>, String> {
    let (base, elem_size) = parse_type_name(&arg.type_name)?;
    let byte_len = arg.len
        .checked_mul(elem_size)
        .ok_or_else(|| "output size overflow".to_string())?;
    let bytes = program.read_global(arg.addr, byte_len)?;
    let mut out = Vec::with_capacity(arg.len);
    for i in 0..arg.len {
        let offset = i * elem_size;
        let value = match base {
            "u8" => format!("{}", bytes[offset]),
            "i8" => format!("{}", bytes[offset] as i8),
            "u16" => {
                let v = u16::from_le_bytes([bytes[offset], bytes[offset + 1]]);
                format!("{}", v)
            }
            "i16" => {
                let v = i16::from_le_bytes([bytes[offset], bytes[offset + 1]]);
                format!("{}", v)
            }
            "u32" => {
                let v = u32::from_le_bytes([
                    bytes[offset],
                    bytes[offset + 1],
                    bytes[offset + 2],
                    bytes[offset + 3],
                ]);
                format!("{}", v)
            }
            "i32" => {
                let v = i32::from_le_bytes([
                    bytes[offset],
                    bytes[offset + 1],
                    bytes[offset + 2],
                    bytes[offset + 3],
                ]);
                format!("{}", v)
            }
            "u64" => {
                let v = u64::from_le_bytes([
                    bytes[offset],
                    bytes[offset + 1],
                    bytes[offset + 2],
                    bytes[offset + 3],
                    bytes[offset + 4],
                    bytes[offset + 5],
                    bytes[offset + 6],
                    bytes[offset + 7],
                ]);
                format!("{}", v)
            }
            "i64" => {
                let v = i64::from_le_bytes([
                    bytes[offset],
                    bytes[offset + 1],
                    bytes[offset + 2],
                    bytes[offset + 3],
                    bytes[offset + 4],
                    bytes[offset + 5],
                    bytes[offset + 6],
                    bytes[offset + 7],
                ]);
                format!("{}", v)
            }
            "f32" => {
                let v = f32::from_le_bytes([
                    bytes[offset],
                    bytes[offset + 1],
                    bytes[offset + 2],
                    bytes[offset + 3],
                ]);
                format!("{}", v)
            }
            "bf16" => {
                let v = u16::from_le_bytes([bytes[offset], bytes[offset + 1]]);
                format!("{}", bf16::from_bits(v).to_f32())
            }
            _ => return Err(format!("unsupported output type '{}'", base)),
        };
        out.push(value);
    }
    Ok(out)
}

fn parse_type_name(value: &str) -> Result<(&str, usize), String> {
    let base = value.split_once('[').map(|(b, _)| b.trim()).unwrap_or(value.trim());
    let elem_size = match base {
        "u8" | "i8" => 1,
        "u16" | "i16" | "bf16" => 2,
        "u32" | "i32" | "f32" => 4,
        "u64" | "i64" => 8,
        _ => return Err(format!("unsupported type '{}'", base)),
    };
    Ok((base, elem_size))
}

// add debug run here, invoking repl or other stuff

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_program(local: Dim3, global: Dim3, wave: WaveSize) -> Program {
        Program::new(1024, local, global, wave)
    }

    #[test]
    fn test_valid_launch_config() {
        let prog = make_test_program(Dim3::new(64, 1, 1), Dim3::new(64, 1, 1), WaveSize::Wave32);
        assert!(prog.validate_launch_config().is_ok());

        let prog = make_test_program(Dim3::new(256, 1, 1), Dim3::new(512, 1, 1), WaveSize::Wave32);
        assert!(prog.validate_launch_config().is_ok());

        let prog = make_test_program(Dim3::new(8, 8, 1), Dim3::new(16, 16, 1), WaveSize::Wave64);
        assert!(prog.validate_launch_config().is_ok());

        let prog = make_test_program(Dim3::new(16, 1, 1), Dim3::new(64, 1, 1), WaveSize::Wave32);
        assert!(prog.validate_launch_config().is_ok());

        let prog = make_test_program(Dim3::new(17, 1, 1), Dim3::new(100, 1, 1), WaveSize::Wave32);
        assert!(prog.validate_launch_config().is_ok());
    }

    #[test]
    fn test_zero_local_dimensions() {
        let prog = make_test_program(Dim3::new(0, 1, 1), Dim3::new(64, 1, 1), WaveSize::Wave32);
        let result = prog.validate_launch_config();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("local launch size cannot have zero dimensions"));

        let prog = make_test_program(Dim3::new(1, 0, 1), Dim3::new(1, 64, 1), WaveSize::Wave32);
        let result = prog.validate_launch_config();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("local launch size cannot have zero dimensions"));

        let prog = make_test_program(Dim3::new(1, 1, 0), Dim3::new(1, 1, 64), WaveSize::Wave32);
        let result = prog.validate_launch_config();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("local launch size cannot have zero dimensions"));
    }

    #[test]
    fn test_zero_global_dimensions() {
        let prog = make_test_program(Dim3::new(64, 1, 1), Dim3::new(0, 1, 1), WaveSize::Wave32);
        let result = prog.validate_launch_config();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("global launch size cannot have zero dimensions"));

        let prog = make_test_program(Dim3::new(1, 64, 1), Dim3::new(1, 0, 1), WaveSize::Wave32);
        let result = prog.validate_launch_config();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("global launch size cannot have zero dimensions"));

        let prog = make_test_program(Dim3::new(1, 1, 64), Dim3::new(1, 1, 0), WaveSize::Wave32);
        let result = prog.validate_launch_config();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("global launch size cannot have zero dimensions"));
    }

    #[test]
    fn test_global_must_be_greater_or_equal() {
        let prog = make_test_program(Dim3::new(64, 1, 1), Dim3::new(32, 1, 1), WaveSize::Wave32);
        let result = prog.validate_launch_config();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("global size x"));
        assert!(err.contains("must be >= local size x"));

        let prog = make_test_program(Dim3::new(1, 32, 1), Dim3::new(1, 16, 1), WaveSize::Wave32);
        let result = prog.validate_launch_config();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("global size y"));
        assert!(err.contains("must be >= local size y"));

        let prog = make_test_program(Dim3::new(1, 1, 16), Dim3::new(1, 1, 8), WaveSize::Wave32);
        let result = prog.validate_launch_config();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("global size z"));
        assert!(err.contains("must be >= local size z"));
    }

    #[test]
    fn test_thread_limit_per_workgroup() {
        let prog = make_test_program(Dim3::new(256, 1, 1), Dim3::new(256, 1, 1), WaveSize::Wave32);
        assert!(prog.validate_launch_config().is_ok());

        let prog = make_test_program(Dim3::new(257, 1, 1), Dim3::new(257, 1, 1), WaveSize::Wave32);
        let result = prog.validate_launch_config();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("produces 257 threads per workgroup"));
        assert!(err.contains("max 256 for RDNA 3.5"));

        let prog = make_test_program(Dim3::new(16, 16, 2), Dim3::new(32, 32, 4), WaveSize::Wave64);
        let result = prog.validate_launch_config();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("produces 512 threads per workgroup"));
    }
}
