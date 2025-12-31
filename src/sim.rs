use crate::isa::InstructionCommonDef;
use crate::wave::WaveState;
use crate::Program;

#[derive(Clone, Debug)]
pub struct GlobalAlloc {
    memory: Vec<u8>,
    next: usize,
}

impl GlobalAlloc {
    pub fn new(size: usize) -> Self {
        Self {
            memory: vec![0; size],
            next: 0,
        }
    }

    pub fn alloc(&mut self, size: usize, align: usize) -> Result<u64, String> {
        let align = align.max(1);
        let aligned = (self.next + align - 1) / align * align;
        let end = aligned
            .checked_add(size)
            .ok_or_else(|| "global alloc overflow".to_string())?;
        if end > self.memory.len() {
            return Err(format!(
                "global alloc out of memory: need {}, have {}",
                end,
                self.memory.len()
            ));
        }
        self.next = end;
        Ok(aligned as u64)
    }

    pub fn write(&mut self, addr: u64, data: &[u8]) -> Result<(), String> {
        let start = addr as usize;
        let end = start
            .checked_add(data.len())
            .ok_or_else(|| "global write overflow".to_string())?;
        if end > self.memory.len() {
            return Err(format!(
                "global write out of bounds: {}..{} (len {})",
                start,
                end,
                self.memory.len()
            ));
        }
        self.memory[start..end].copy_from_slice(data);
        Ok(())
    }

    pub fn write_zeros(&mut self, addr: u64, size: usize) -> Result<(), String> {
        let start = addr as usize;
        let end = start
            .checked_add(size)
            .ok_or_else(|| "global write overflow".to_string())?;
        if end > self.memory.len() {
            return Err(format!(
                "global write out of bounds: {}..{} (len {})",
                start,
                end,
                self.memory.len()
            ));
        }
        self.memory[start..end].fill(0);
        Ok(())
    }

    pub fn read(&self, addr: u64, size: usize) -> Result<Vec<u8>, String> {
        let start = addr as usize;
        let end = start
            .checked_add(size)
            .ok_or_else(|| "global read overflow".to_string())?;
        if end > self.memory.len() {
            return Err(format!(
                "global read out of bounds: {}..{} (len {})",
                start,
                end,
                self.memory.len()
            ));
        }
        Ok(self.memory[start..end].to_vec())
    }
}

pub fn generate_arange(start: f64, end: f64, step: f64) -> Result<Vec<f64>, String> {
    if step == 0.0 {
        return Err("arange step cannot be 0".to_string());
    }
    let mut out = Vec::new();
    let mut value = start;
    if step > 0.0 {
        while value < end {
            out.push(value);
            value += step;
        }
    } else {
        while value > end {
            out.push(value);
            value += step;
        }
    }
    Ok(out)
}

pub fn generate_matrix(rows: usize, cols: usize, start: f64, step: f64) -> Vec<f64> {
    let count = rows.saturating_mul(cols);
    let mut out = Vec::with_capacity(count);
    for idx in 0..count {
        out.push(start + step * idx as f64);
    }
    out
}

#[derive(Debug)]
pub enum ExecError {
    Unimplemented(&'static str),
}

pub type ExecResult = Result<(), ExecError>;

pub struct DecodedInst;

pub struct ExecContext<'a> {
    pub wave: &'a mut WaveState,
    pub program: &'a mut Program,
}

pub type Handler = fn(&mut ExecContext, &DecodedInst) -> ExecResult;

pub fn dispatch(
    arch_ops: &[(&'static str, Handler)],
    base_ops: &[(&'static str, Handler)],
    def: &InstructionCommonDef,
    ctx: &mut ExecContext,
    decoded: &DecodedInst,
) -> ExecResult {
    if let Ok(idx) = arch_ops.binary_search_by(|(name, _)| name.cmp(&def.name)) {
        return (arch_ops[idx].1)(ctx, decoded);
    }
    if let Ok(idx) = base_ops.binary_search_by(|(name, _)| name.cmp(&def.name)) {
        return (base_ops[idx].1)(ctx, decoded);
    }
    Err(ExecError::Unimplemented(def.name))
}
