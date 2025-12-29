use clap::{CommandFactory, Parser};
use std::path::PathBuf;

use rdna_sim::{run_file, Architecture, WaveSize, Dim3};

#[derive(Parser, Debug)]
#[command(name = "rdna-sim", about = "RDNA/CDNA architecture simulator")]
struct Cli {
  #[arg(short, long, value_enum, default_value_t = Architecture::Rdna35)]
  arch: Architecture,

  #[arg(short, long, value_name = "PATH")]
  file: Option<PathBuf>,

  #[arg(long, value_enum, default_value_t = WaveSize::Wave32)]
  wave_size: WaveSize,

  #[arg(long)]
  debug: bool,

  #[arg(long, default_value="1,1,1")]
  dim: Dim3,
}

fn main() {
  if std::env::args().len() == 1 {
    let mut cmd = Cli::command();
    cmd.print_help().expect("print help");
    println!();
    return;
  }
  let args = Cli::parse();
  if let Err(err) = run_file(args.file, args.arch, args.wave_size, args.debug) {
    eprintln!("{}", err);
    std::process::exit(1);
  }
}
