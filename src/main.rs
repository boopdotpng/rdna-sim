use clap::{CommandFactory, Parser};
use std::path::PathBuf;

use rdna_sim::{run_file, Architecture, WaveSize};

#[derive(Parser, Debug)]
#[command(name = "rdna-sim", about = "rdna/cdna architecture simulator")]
struct Cli {
  #[arg(
    short,
    long,
    value_enum,
    default_value_t = Architecture::Rdna35,
    help = "Target architecture"
  )]
  arch: Architecture,

  #[arg(value_name = "PATH", help = "Path to the RDNA assembly file")]
  file: Option<PathBuf>,

  #[arg(
    short = 'w',
    long,
    value_enum,
    default_value_t = WaveSize::Wave32,
    help = "Wavefront size (32 only for now)"
  )]
  wave_size: WaveSize,

  // launch TUI debugger, when implemented
  #[arg(short, long, help = "Launch the debugger (not implemented yet)")]
  debug: bool,

  // suppress print instructions inside the kernel
  #[arg(short = 's', long, help = "Suppress kernel print directives")]
  suppress_prints: bool,

  #[arg(
    short = 'm',
    long = "global-memsize",
    value_name = "MEGABYTES",
    default_value_t = 32,
    help = "Global memory size in megabytes"
  )]
  global_memsize: usize,

}

fn main() {
  if std::env::args().len() == 1 {
    let mut cmd = Cli::command();
    cmd.print_help().expect("print help");
    println!();
    return;
  }
  let args = Cli::parse();
  let global_mem_bytes = args.global_memsize * 1024 * 1024; // MB to bytes 
  if let Err(err) = run_file(
    args.file,
    args.arch,
    args.wave_size,
    global_mem_bytes,
    args.debug,
  ) {
    eprintln!("{}", err);
    std::process::exit(1);
  }
}
