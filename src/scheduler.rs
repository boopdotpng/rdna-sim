use crate::ops::{base, rdna35};
use crate::parse::ProgramInfo;
use crate::sim::{dispatch, Handler, KernArg, LDS};
use crate::wave::{WaveState, VGPR_MAX};
use crate::{Architecture, Program, WaveSize};

pub fn run_program(
  program: &mut Program,
  info: &ProgramInfo,
  arch: Architecture,
) -> Result<(), String> {
  let arch_ops = select_arch_ops(arch);
  let base_ops = base::OPS;
  let wave_size = info.wave_size.unwrap_or(program.wave_size);

  let kernarg = init_kernarg(program, info)?;
  let total_workgroups = info.global_launch_size.linear_len();
  let threads_per_wg = info.local_launch_size.linear_len() as usize;
  let wave_lanes = match wave_size {
    WaveSize::Wave32 => 32,
    WaveSize::Wave64 => 64,
  };
  let waves_per_wg = (threads_per_wg + wave_lanes - 1) / wave_lanes;

  for wg_id in 0..total_workgroups {
    let mut lds = LDS::new(64 * 1024);
    for wave_idx in 0..waves_per_wg {
      let mut wave = init_wave_state(
        info,
        wave_size,
        &kernarg,
        wg_id,
        wave_idx,
        wave_lanes,
        threads_per_wg,
      )?;
      run_wave(&mut wave, &mut lds, program, info, arch_ops, base_ops)?;
    }
  }

  Ok(())
}

fn select_arch_ops(arch: Architecture) -> &'static [(&'static str, Handler)] {
  match arch {
    Architecture::Rdna35 => rdna35::OPS,
  }
}

fn init_kernarg(program: &mut Program, info: &ProgramInfo) -> Result<KernArg, String> {
  let mut args = Vec::new();
  for arg in info.arguments.iter() {
    args.push(arg.addr);
  }
  for arg in info.output_arguments.iter() {
    args.push(arg.addr);
  }
  KernArg::new(program, &args)
}

fn init_wave_state(
  info: &ProgramInfo,
  wave_size: WaveSize,
  kernarg: &KernArg,
  wg_id: u64,
  wave_idx: usize,
  wave_lanes: usize,
  threads_per_wg: usize,
) -> Result<WaveState, String> {
  let (wg_x, wg_y, wg_z) = info.global_launch_size.split_linear(wg_id);
  let wave_start = wave_idx * wave_lanes;
  let active_lanes = threads_per_wg.saturating_sub(wave_start).min(wave_lanes);
  let exec_mask = build_exec_mask(wave_size, active_lanes);
  let mut wave = WaveState::new(wave_size, VGPR_MAX, exec_mask)?;
  wave.write_sgpr_pair(0, kernarg.base);
  wave.write_sgpr(2, wg_x);
  if info.global_launch_size.1 > 1 {
    wave.write_sgpr(3, wg_y);
  }
  if info.global_launch_size.2 > 1 {
    wave.write_sgpr(4, wg_z);
  }
  let pack_local_ids = info.local_launch_size.1 > 1 || info.local_launch_size.2 > 1;
  for lane in 0..active_lanes {
    let local_tid = (wave_start + lane) as u64;
    let (local_x, local_y, local_z) = info.local_launch_size.split_linear(local_tid);
    if pack_local_ids {
      let packed = (local_x & 0x3FF)
        | ((local_y & 0x3FF) << 10)
        | ((local_z & 0x3FF) << 20);
      wave.write_vgpr(0, lane, packed);
    } else {
      wave.write_vgpr(0, lane, local_x);
    }
  }
  Ok(wave)
}

fn build_exec_mask(wave_size: WaveSize, active_lanes: usize) -> u64 {
  if active_lanes == 0 {
    return 0;
  }
  match wave_size {
    WaveSize::Wave32 => {
      if active_lanes >= 32 {
        0xFFFF_FFFF
      } else {
        (1u64 << active_lanes) - 1
      }
    }
    WaveSize::Wave64 => {
      if active_lanes >= 64 {
        u64::MAX
      } else {
        (1u64 << active_lanes) - 1
      }
    }
  }
}

fn run_wave(
  wave: &mut WaveState,
  lds: &mut LDS,
  program: &mut Program,
  info: &ProgramInfo,
  arch_ops: &[(&'static str, Handler)],
  base_ops: &[(&'static str, Handler)],
) -> Result<(), String> {
  loop {
    if wave.is_halted() {
      return Ok(());
    }
    let pc = wave.pc() as usize;
    if pc >= info.instructions.len() {
      return Err(format!("pc {} out of range", pc));
    }
    wave.apply_pending_counters();
    let inst = &info.instructions[pc];
    if handle_waitcnt(wave, inst) {
      continue;
    }
    match dispatch(arch_ops, base_ops, inst.def, wave, lds, program, inst) {
      Ok(()) => {}
      Err(crate::sim::ExecError::EndProgram) => return Ok(()),
      Err(e) => return Err(format!("line {}: {:?}", inst.line_num, e)),
    }
    wave.increment_pc(1);
  }
}

fn handle_waitcnt(wave: &mut WaveState, inst: &crate::sim::DecodedInst) -> bool {
  let Some(targets) = wait_targets(inst) else {
    return false;
  };
  if let Some(target) = targets.vmcnt {
    if wave.vmcnt() > target {
      return true;
    }
  }
  if let Some(target) = targets.vscnt {
    if wave.vscnt() > target {
      return true;
    }
  }
  if let Some(target) = targets.lgkmcnt {
    if wave.lgkmcnt() > target {
      return true;
    }
  }
  if let Some(target) = targets.expcnt {
    if wave.expcnt() > target {
      return true;
    }
  }
  wave.increment_pc(1);
  true
}

struct WaitTargets {
  vmcnt: Option<u8>,
  vscnt: Option<u8>,
  lgkmcnt: Option<u8>,
  expcnt: Option<u8>,
}

fn wait_targets(inst: &crate::sim::DecodedInst) -> Option<WaitTargets> {
  let imm = match inst.operands.get(0) {
    Some(crate::sim::DecodedOperand::ImmU32(v)) => *v,
    _ => 0,
  };
  match inst.name.as_str() {
    "s_waitcnt" => {
      let vmcnt = ((imm >> 10) & 0x3F) as u8;
      let lgkmcnt = ((imm >> 4) & 0x3F) as u8;
      let expcnt = (imm & 0x7) as u8;
      Some(WaitTargets {
        vmcnt: Some(vmcnt),
        vscnt: None,
        lgkmcnt: Some(lgkmcnt),
        expcnt: Some(expcnt),
      })
    }
    "s_waitcnt_vmcnt" => Some(WaitTargets {
      vmcnt: Some(imm as u8),
      vscnt: None,
      lgkmcnt: None,
      expcnt: None,
    }),
    "s_waitcnt_vscnt" => Some(WaitTargets {
      vmcnt: None,
      vscnt: Some(imm as u8),
      lgkmcnt: None,
      expcnt: None,
    }),
    "s_waitcnt_lgkmcnt" => Some(WaitTargets {
      vmcnt: None,
      vscnt: None,
      lgkmcnt: Some(imm as u8),
      expcnt: None,
    }),
    "s_waitcnt_expcnt" => Some(WaitTargets {
      vmcnt: None,
      vscnt: None,
      lgkmcnt: None,
      expcnt: Some(imm as u8),
    }),
    _ => None,
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::Dim3;
  use crate::parse::ArgInfo;
  use crate::parse_instruction::SpecialRegister;

  fn program_info(local: Dim3, global: Dim3, args: Vec<ArgInfo>, outputs: Vec<ArgInfo>) -> ProgramInfo {
    ProgramInfo {
      instructions: Vec::new(),
      arguments: args,
      output_arguments: outputs,
      local_launch_size: local,
      global_launch_size: global,
      wave_size: Some(WaveSize::Wave32),
    }
  }

  fn alloc_arg(program: &mut Program, name: &str, size: usize) -> ArgInfo {
    let addr = program.global_mem.alloc(size, 8).expect("alloc");
    ArgInfo {
      name: name.to_string(),
      type_name: "u64".to_string(),
      shape: Vec::new(),
      addr,
      len: 1,
    }
  }

  #[test]
  fn kernarg_base_is_written_when_present() {
    let mut program = Program::new(1024, Dim3::new(1, 1, 1), Dim3::new(1, 1, 1), WaveSize::Wave32);
    let args = vec![alloc_arg(&mut program, "arg0", 8)];
    let outputs = vec![alloc_arg(&mut program, "out0", 8)];
    let info = program_info(Dim3::new(1, 1, 1), Dim3::new(1, 1, 1), args, outputs);
    let kernarg = init_kernarg(&mut program, &info).expect("kernarg");

    let wave = init_wave_state(
      &info,
      WaveSize::Wave32,
      &kernarg,
      0,
      0,
      32,
      info.local_launch_size.linear_len() as usize,
    )
    .expect("init wave");

    assert_eq!(wave.read_sgpr_pair(0), kernarg.base);
  }

  #[test]
  fn kernarg_base_is_zero_when_absent() {
    let mut program = Program::new(1024, Dim3::new(1, 1, 1), Dim3::new(1, 1, 1), WaveSize::Wave32);
    let info = program_info(Dim3::new(1, 1, 1), Dim3::new(1, 1, 1), Vec::new(), Vec::new());
    let kernarg = init_kernarg(&mut program, &info).expect("kernarg");

    let wave = init_wave_state(
      &info,
      WaveSize::Wave32,
      &kernarg,
      0,
      0,
      32,
      info.local_launch_size.linear_len() as usize,
    )
    .expect("init wave");

    assert_eq!(wave.read_sgpr_pair(0), 0);
  }

  #[test]
  fn workgroup_ids_are_written_to_sgprs() {
    let mut program = Program::new(1024, Dim3::new(1, 1, 1), Dim3::new(2, 3, 4), WaveSize::Wave32);
    let info = program_info(Dim3::new(1, 1, 1), Dim3::new(2, 3, 4), Vec::new(), Vec::new());
    let kernarg = init_kernarg(&mut program, &info).expect("kernarg");

    let wg_id = 5;
    let (wg_x, wg_y, wg_z) = info.global_launch_size.split_linear(wg_id);
    let wave = init_wave_state(
      &info,
      WaveSize::Wave32,
      &kernarg,
      wg_id,
      0,
      32,
      info.local_launch_size.linear_len() as usize,
    )
    .expect("init wave");

    assert_eq!(wave.read_sgpr::<u32>(2), wg_x);
    assert_eq!(wave.read_sgpr::<u32>(3), wg_y);
    assert_eq!(wave.read_sgpr::<u32>(4), wg_z);
  }

  #[test]
  fn local_ids_are_packed_in_vgpr0_for_2d_or_3d() {
    let mut program = Program::new(1024, Dim3::new(2, 2, 2), Dim3::new(1, 1, 1), WaveSize::Wave32);
    let info = program_info(Dim3::new(2, 2, 2), Dim3::new(1, 1, 1), Vec::new(), Vec::new());
    let kernarg = init_kernarg(&mut program, &info).expect("kernarg");
    let wave_lanes = 32;
    let threads_per_wg = info.local_launch_size.linear_len() as usize;

    let wave = init_wave_state(
      &info,
      WaveSize::Wave32,
      &kernarg,
      0,
      0,
      wave_lanes,
      threads_per_wg,
    )
    .expect("init wave");

    for lane in 0..threads_per_wg {
      let (x, y, z) = info.local_launch_size.split_linear(lane as u64);
      let expected = (x & 0x3FF) | ((y & 0x3FF) << 10) | ((z & 0x3FF) << 20);
      assert_eq!(wave.read_vgpr(0, lane), expected);
    }
  }

  #[test]
  fn local_ids_are_packed_in_vgpr0_for_3d() {
    let mut program = Program::new(1024, Dim3::new(3, 4, 2), Dim3::new(1, 1, 1), WaveSize::Wave32);
    let info = program_info(Dim3::new(3, 4, 2), Dim3::new(1, 1, 1), Vec::new(), Vec::new());
    let kernarg = init_kernarg(&mut program, &info).expect("kernarg");
    let wave_lanes = 32;
    let threads_per_wg = info.local_launch_size.linear_len() as usize;

    let wave = init_wave_state(
      &info,
      WaveSize::Wave32,
      &kernarg,
      0,
      0,
      wave_lanes,
      threads_per_wg,
    )
    .expect("init wave");

    for lane in 0..threads_per_wg {
      let (x, y, z) = info.local_launch_size.split_linear(lane as u64);
      let expected = (x & 0x3FF) | ((y & 0x3FF) << 10) | ((z & 0x3FF) << 20);
      assert_eq!(wave.read_vgpr(0, lane), expected);
    }
  }

  #[test]
  fn local_ids_use_vgpr0_for_1d() {
    let mut program = Program::new(1024, Dim3::new(8, 1, 1), Dim3::new(1, 1, 1), WaveSize::Wave32);
    let info = program_info(Dim3::new(8, 1, 1), Dim3::new(1, 1, 1), Vec::new(), Vec::new());
    let kernarg = init_kernarg(&mut program, &info).expect("kernarg");
    let wave_lanes = 32;
    let threads_per_wg = info.local_launch_size.linear_len() as usize;

    let wave = init_wave_state(
      &info,
      WaveSize::Wave32,
      &kernarg,
      0,
      0,
      wave_lanes,
      threads_per_wg,
    )
    .expect("init wave");

    for lane in 0..threads_per_wg {
      let (x, _, _) = info.local_launch_size.split_linear(lane as u64);
      assert_eq!(wave.read_vgpr(0, lane), x);
    }
  }

  #[test]
  fn workgroup_ids_are_written_to_sgprs_per_wave_in_3d() {
    let mut program = Program::new(1024, Dim3::new(1, 1, 1), Dim3::new(2, 3, 4), WaveSize::Wave32);
    let info = program_info(Dim3::new(1, 1, 1), Dim3::new(2, 3, 4), Vec::new(), Vec::new());
    let kernarg = init_kernarg(&mut program, &info).expect("kernarg");
    let wave_lanes = 32;
    let threads_per_wg = info.local_launch_size.linear_len() as usize;
    let wg_id = 7;
    let (wg_x, wg_y, wg_z) = info.global_launch_size.split_linear(wg_id);

    let wave0 = init_wave_state(
      &info,
      WaveSize::Wave32,
      &kernarg,
      wg_id,
      0,
      wave_lanes,
      threads_per_wg,
    )
    .expect("init wave");
    let wave1 = init_wave_state(
      &info,
      WaveSize::Wave32,
      &kernarg,
      wg_id,
      1,
      wave_lanes,
      threads_per_wg,
    )
    .expect("init wave");

    assert_eq!(wave0.read_sgpr::<u32>(2), wg_x);
    assert_eq!(wave0.read_sgpr::<u32>(3), wg_y);
    assert_eq!(wave0.read_sgpr::<u32>(4), wg_z);
    assert_eq!(wave1.read_sgpr::<u32>(2), wg_x);
    assert_eq!(wave1.read_sgpr::<u32>(3), wg_y);
    assert_eq!(wave1.read_sgpr::<u32>(4), wg_z);
  }

  #[test]
  fn partial_wave_exec_mask_prevents_vgpr_writes() {
    let mut program = Program::new(1024, Dim3::new(16, 1, 1), Dim3::new(64, 1, 1), WaveSize::Wave32);
    let info = program_info(Dim3::new(16, 1, 1), Dim3::new(64, 1, 1), Vec::new(), Vec::new());
    let kernarg = init_kernarg(&mut program, &info).expect("kernarg");
    let wave_lanes = 32;
    let threads_per_wg = info.local_launch_size.linear_len() as usize;

    let wave = init_wave_state(
      &info,
      WaveSize::Wave32,
      &kernarg,
      0,
      0,
      wave_lanes,
      threads_per_wg,
    )
    .expect("init wave");

    for lane in 0..16 {
      assert_eq!(wave.read_vgpr(0, lane), lane as u32);
    }
    for lane in 16..32 {
      assert_eq!(wave.read_vgpr(0, lane), 0);
    }
  }

  #[test]
  fn exec_mask_matches_active_lanes() {
    let mut program = Program::new(1024, Dim3::new(1, 1, 1), Dim3::new(16, 1, 1), WaveSize::Wave32);
    let info = program_info(Dim3::new(16, 1, 1), Dim3::new(1, 1, 1), Vec::new(), Vec::new());
    let kernarg = init_kernarg(&mut program, &info).expect("kernarg");

    let wave = init_wave_state(
      &info,
      WaveSize::Wave32,
      &kernarg,
      0,
      0,
      32,
      info.local_launch_size.linear_len() as usize,
    )
    .expect("init wave");
    assert_eq!(wave.read_special_b32(SpecialRegister::ExecLo), 0x0000_FFFF);

    let mut program = Program::new(1024, Dim3::new(1, 1, 1), Dim3::new(32, 1, 1), WaveSize::Wave32);
    let info = program_info(Dim3::new(32, 1, 1), Dim3::new(1, 1, 1), Vec::new(), Vec::new());
    let kernarg = init_kernarg(&mut program, &info).expect("kernarg");
    let wave = init_wave_state(
      &info,
      WaveSize::Wave32,
      &kernarg,
      0,
      0,
      32,
      info.local_launch_size.linear_len() as usize,
    )
    .expect("init wave");
    assert_eq!(wave.read_special_b32(SpecialRegister::ExecLo), 0xFFFF_FFFF);
  }
}
