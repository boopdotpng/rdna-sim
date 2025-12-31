// Parsing logic for RDNA assembly files
use std::fs;
use std::path::Path;
use std::str::FromStr;

use crate::{Program, WaveSize, Architecture, Dim3};

pub struct ProgramInfo {
    pub instructions: Vec<String>,
    pub arguments: Vec<(String, String)>,  // (name, type)
    pub output_arguments: Vec<(String, String)>,  // (name, type) for output args
    pub print_addresses: Vec<(u64, String)>, // (address, type)
    pub local_launch_size: Dim3,
    pub global_launch_size: Dim3,
}

pub fn parse_top(file_path: &Path) -> Result<ProgramInfo, String> {
    // Read the entire file content
    let content = fs::read_to_string(file_path)
        .map_err(|e| format!("Failed to read file {}: {}", file_path.display(), e))?;
    
    // Split content into lines
    let lines: Vec<&str> = content.lines().collect();
    
    let mut instructions = Vec::new();
    let mut arguments = Vec::new();
    let mut output_arguments = Vec::new();
    let mut print_addresses = Vec::new();
    let mut in_args_section = false;
    let mut in_print_section = false;
    let mut local_launch_size = Dim3::new(1, 1, 1);
    let mut global_launch_size = Dim3::new(1, 1, 1);
    
    // Parse the file based on the documented format
    for line in lines {
        let line = line.trim();
        
        // Skip empty lines and comments
        if line.is_empty() || line.starts_with("//") {
            continue;
        }
        
        // Check for section markers
        if line == "---" {
            in_args_section = !in_args_section;
            in_print_section = false;
            continue;
        }
        
        if line == "printing section" {
            in_print_section = true;
            in_args_section = false;
            continue;
        }
        
        // Parse launch size sections
        if line.starts_with("local:") {
            if let Some(size_str) = line.split(':').nth(1) {
                if let Ok(dim) = Dim3::from_str(size_str.trim()) {
                    local_launch_size = dim;
                }
            }
            continue;
        }
        
        if line.starts_with("global:") {
            if let Some(size_str) = line.split(':').nth(1) {
                if let Ok(dim) = Dim3::from_str(size_str.trim()) {
                    global_launch_size = dim;
                }
            }
            continue;
        }
        
        if in_args_section {
            // Parse arguments section - only process lines that contain a colon
            if line.contains(':') {
                if let Some((name, type_str)) = line.split_once(':') {
                    let name = name.trim().to_string();
                    let type_str = type_str.trim().to_string();
                    
                    // Check if this is an output argument (starts with "out_")
                    if name.starts_with("out_") {
                        output_arguments.push((name, type_str));
                    } else {
                        arguments.push((name, type_str));
                    }
                }
            } else {
                // This is probably an instruction
                instructions.push(line.to_string());
            }
        } else if in_print_section {
            // Parse print section
            if let Some((addr_str, type_str)) = line.split_once(':') {
                if let Ok(addr) = u64::from_str_radix(addr_str.trim().trim_start_matches("0x"), 16) {
                    let type_str = type_str.trim().to_string();
                    print_addresses.push((addr, type_str));
                }
            }
        } else {
            // Regular instruction line
            instructions.push(line.to_string());
        }
    }
    
    Ok(ProgramInfo {
        instructions,
        arguments,
        output_arguments,
        print_addresses,
        local_launch_size,
        global_launch_size,
    })
}

pub fn parse_into_program(program_info: ProgramInfo, _arch: Architecture, _wave_size: WaveSize) -> Program {
    // Create program with the parsed information
    Program {
        global_mem: vec![0u64; 8192].into_boxed_slice(),
        local_launch_size: program_info.local_launch_size,
        global_launch_size: program_info.global_launch_size,
    }
}