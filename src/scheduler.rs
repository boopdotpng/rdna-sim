use crate::ops::{base, rdna35};
use crate::parse::ProgramInfo;
use crate::sim::{dispatch, ExecContext, Handler, KernArg, LDS};
use crate::wave::{WaveState, VGPR_MAX};
use crate::{Architecture, Dim3, Program, WaveSize};

pub fn run_program(
  program: &mut Program,
  info: &ProgramInfo,
  arch: Architecture,
) -> Result<(), String> {
  let arch_ops = select_arch_ops(arch);
  let base_ops = base::OPS;
  let wave_size = info.wave_size.unwrap_or(program.wave_size);

  let kernarg = init_kernarg(program, info)?;
  let total_workgroups = mul_dim3(&info.global_launch_size);
  let threads_per_wg = mul_dim3(&info.local_launch_size) as usize;
  let wave_lanes = match wave_size {
    WaveSize::Wave32 => 32,
    WaveSize::Wave64 => 64,
  };
  let waves_per_wg = (threads_per_wg + wave_lanes - 1) / wave_lanes;

  for wg_id in 0..total_workgroups {
    let mut lds = LDS::new(64 * 1024);
    let (wg_x, wg_y, wg_z) = split_dim3(wg_id, &info.global_launch_size);
    for wave_idx in 0..waves_per_wg {
      let wave_start = wave_idx * wave_lanes;
      let active_lanes = threads_per_wg.saturating_sub(wave_start).min(wave_lanes);
      let exec_mask = build_exec_mask(wave_size, active_lanes);
      let mut wave = WaveState::new(wave_size, VGPR_MAX, exec_mask)?;
      wave.write_sgpr(0, wg_x);
      if info.global_launch_size.1 > 1 {
        wave.write_sgpr(1, wg_y);
      }
      if info.global_launch_size.2 > 1 {
        wave.write_sgpr(2, wg_z);
      }
      if !kernarg.is_empty() {
        wave.write_sgpr_pair(3, kernarg.base);
      }
      for lane in 0..active_lanes {
        let local_tid = (wave_start + lane) as u64;
        let (local_x, local_y, local_z) = split_dim3(local_tid, &info.local_launch_size);
        wave.write_vgpr(0, lane, local_x);
        if info.local_launch_size.1 > 1 {
          wave.write_vgpr(1, lane, local_y);
        }
        if info.local_launch_size.2 > 1 {
          wave.write_vgpr(2, lane, local_z);
        }
      }
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

fn mul_dim3(dim: &Dim3) -> u64 {
  dim.0 as u64 * dim.1 as u64 * dim.2 as u64
}

fn split_dim3(linear: u64, dim: &Dim3) -> (u32, u32, u32) {
  let x = (linear % dim.0 as u64) as u32;
  let y = ((linear / dim.0 as u64) % dim.1 as u64) as u32;
  let z = (linear / (dim.0 as u64 * dim.1 as u64)) as u32;
  (x, y, z)
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
    let mut ctx = ExecContext { wave, lds, program };
    dispatch(arch_ops, base_ops, inst.def, &mut ctx, inst)
      .map_err(|e| format!("line {}: {:?}", inst.line_num, e))?;
    if ctx.wave.is_halted() {
      return Ok(());
    }
    ctx.wave.increment_pc(1);
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
