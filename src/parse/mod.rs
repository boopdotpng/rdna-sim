mod args;
mod file;
mod init;

pub use args::{ArgInfo, ProgramInfo};
pub use file::parse_file;

#[cfg(test)]
pub(super) mod test_support {
  use std::fs;
  use std::path::PathBuf;
  use std::sync::atomic::{AtomicUsize, Ordering};

  use super::args::{parse_type, TypeSpec};
  use super::init::Number;
  use crate::{Dim3, Program, WaveSize};

  static TEMP_COUNTER: AtomicUsize = AtomicUsize::new(0);

  pub(super) fn temp_path(name: &str) -> PathBuf {
    let id = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let mut path = std::env::temp_dir();
    path.push(format!(
      "rdna_parse_test_{}_{}_{}.rdna",
      std::process::id(),
      name,
      id
    ));
    path
  }

  pub(super) fn write_temp(contents: &str, name: &str) -> PathBuf {
    let path = temp_path(name);
    fs::write(&path, contents).expect("write temp rdna");
    path
  }

  pub(super) fn program() -> Program {
    Program::new(1024, Dim3::new(1, 1, 1), Dim3::new(1, 1, 1), WaveSize::Wave32)
  }

  pub(super) fn spec(name: &str) -> TypeSpec {
    parse_type(name).expect("type spec")
  }

  pub(super) fn numbers_to_f32(values: &[Number]) -> Vec<f32> {
    values
      .iter()
      .map(|value| match *value {
        Number::Int(v) => v as f32,
        Number::Float(v) => v,
      })
      .collect()
  }

  pub(super) fn numbers_to_i64(values: &[Number]) -> Vec<i64> {
    values
      .iter()
      .map(|value| match *value {
        Number::Int(v) => v,
        Number::Float(v) => v as i64,
      })
      .collect()
  }
}
