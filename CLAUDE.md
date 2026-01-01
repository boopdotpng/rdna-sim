# RDNA Simulator Specification

## Table of Contents
1. [Input File Format](#input-file-format)
2. [ISA Generation & Code Organization](#isa-generation--code-organization)
3. [Instruction Pipeline](#instruction-pipeline)
4. [Project Goals](#project-goals)
5. [Background: RDNA vs CDNA](#background-rdna-vs-cdna)

---

## Input File Format

RDNA simulator input files consist of a YAML-like header block and an instruction block.

### Header Block

The header lives between two `---` lines and declares kernel arguments and launch configuration.

**Example:**
```
---
arg_a: f32[16] = arange(16)
arg_b: i32 = 3
arg_weights: f32[4,4] = rand()
out_y: f32[4,4]
local = N, 1, 1
global = N, 1, 1
wave = 32
---
```

#### Type System
- **Scalar**: `name: type` (e.g., `counter: i32`)
- **Array**: `name: type[shape]` (e.g., `data: f32[16]` or `matrix: f32[4,4]`)
- **Supported types**: `u8`, `u16`, `u32`, `u64`, `i8`, `i16`, `i32`, `i64`, `f32`, `bf16`
- **Shapes**: Single dimension `[n]` or multi-dimensional `[r,c,...]` (row-major layout)

#### Initializers

Initializers are optional. If omitted, arguments are zero-initialized.

**Literal values:**
```
scalars: i32[4] = 1, 2, 3, 4
matrix: f32[2,2] = 1.0, 2.0, 3.0, 4.0
```

**Built-in functions:**
- `repeat(value)` - Fill array with single value
- `arange(n)` or `arange(start, end[, step])` - Sequential range
- `rand()` - Random values (floats in [0.0, 1.0), integers in [0, 100))
- `file("path", dtype)` - Load raw binary data from file

**Number formats:**
- Decimal: `42`, `3.14`
- Hexadecimal: `0xDEADBEEF`
- Binary: `0b1010`
- Floats: standard decimal form

#### Launch Configuration

```
local = x, y, z      # Threads per workgroup (CUDA block equivalent)
global = x, y, z     # Number of workgroups (CUDA grid equivalent)
wave = 32            # Wave size (32 or 64)
```

Parentheses are optional: `local = (x, y, z)` is equivalent to `local = x, y, z`.

#### Output Arguments

Variables prefixed with `out_` are printed after kernel execution:
```
out_result: f32[16]    # Allocated and printed after execution
```

#### Memory Layout

Arguments are allocated in global memory with a 64-bit kernarg pointer placed in SGPRs. The kernarg table is a contiguous list of `u64` addresses (inputs first, then outputs). When args exist, `s[3:4]` holds the table pointer.

**Thread/workgroup IDs:**
- Workgroup IDs: `s0`, `s1`, `s2` (only present if global size has those dimensions)
- Local (thread) IDs: `v0`, `v1`, `v2` per lane (only present if local size has those dimensions)

### Instruction Block

Instructions follow standard RDNA assembly syntax: one opcode followed by comma-separated operands.

#### Operand Forms

**Registers:**
- Scalar: `sN`, `s[lo:hi]` (e.g., `s0`, `s[2:5]`)
- Vector: `vN`, `v[lo:hi]` (e.g., `v0`, `v[0:3]`)

**Special registers:**
- `vcc`, `vcc_lo`, `vcc_hi` - Vector condition code
- `exec`, `exec_lo`, `exec_hi` - Execution mask
- `m0` - Memory dependency counter
- `null` - Discard destination
- `scc` - Scalar condition code

**Immediates:**
- Decimal, hexadecimal (`0x`), binary (`0b`), or floating-point
- Negative values allowed: `-42`, `-3.14`

**Modifiers:**
- Negation: `-v0`, `-1.0`
- Absolute value: `|v0|`
- Combined: `-|v0|` (abs then negate)
- **Restriction**: Modifiers not allowed on register ranges

**Memory operands:**
- `offset:NN` - Memory offset (e.g., `offset:16`)

**Flags:**
- Memory flags: `glc`, `slc`, `nt`, `offen`, `idxen`
- Do not count toward operand count

#### Special Instruction Syntax

**s_waitcnt**: Accepts named arguments or raw immediate
```
s_waitcnt lgkmcnt(0) vmcnt(0)
s_waitcnt 0x0000
```

**s_sendmsg**: Uses sendmsg() syntax
```
s_sendmsg sendmsg(MSG_GS_DONE, GS_OP_NOP)
```

#### Print Directives (Planned)

Lines starting with `print` are parsed but currently ignored during execution.

**Planned syntax:**
```
print [wave=<id>,] [thread=<id>|thread=all,] <arg>[, <arg>...]
```

Where `<arg>` can be `s[n]`, `s[lo:hi]`, `v[n]`, `exec`, `vcc`, or `scc`.

### Current Limitations

- `wave = 64` is parsed but currently rejected at load time
- `print` directives are parsed but not executed
- F64 instructions deliberately excluded (too slow for compute workloads)

### Global Memory

- Parsing allocates each argument in global memory immediately
- Backed by a simple bump allocator with byte-level read/write helpers
- `bf16` values are encoded properly during initialization
- CLI flag: `--global-memsize <MB>` (default: 32MB)

---

## ISA Generation & Code Organization

### ISA XML Sources

ISA definitions live in `data/` directory:
- `data/isa_rdna3.xml` - RDNA 3 instruction set
- `data/isa_rdna35.xml` - RDNA 3.5 instruction set
- `data/isa_rdna4.xml` - RDNA 4 instruction set

The `scripts/fetch_isa.sh` script downloads these XMLs, deliberately excluding older architectures (RDNA1/2, CDNA1/2) that are out of scope.

### Code Generation Strategy

The `scripts/gen_isa.py` script parses ISA XMLs and generates:

1. **ISA metadata** in `src/isa/{base,rdna3,rdna35,rdna4}/generated.rs`
   - Common instruction definitions
   - Architecture-specific instruction definitions
   - Operand specifications and type information
   - Encoding metadata (VOP3, SDWA, etc.)

2. **Operation stubs** in `src/ops/{base,rdna3,rdna35,rdna4}/`
   - Only regenerated when `--write-ops` flag is set
   - Prevents clobbering custom handler implementations

#### Instruction Deduplication

**Two-layer approach:**

1. **Common (base) instructions**: Instructions with identical signatures across all architectures
   - Name + operand specs + encoding list must match exactly
   - Stored once in `src/isa/base/generated.rs`
   - Referenced by arch-specific modules

2. **Architecture-specific instructions**: Instructions unique to or modified in specific architectures
   - Stored in `src/isa/{arch}/generated.rs`
   - Include new instructions or variant forms

**Generation output:**
```
base: 823 instructions
rdna3: 986 instructions (163 arch-specific)
rdna3.5: 1058 instructions (235 arch-specific)
rdna4: 1073 instructions (250 arch-specific)
```

### Operation Handler Organization

Handlers are categorized by instruction type into separate files:

```
src/ops/base/
├── mod.rs           # Module declarations + OPS lookup table
├── vector.rs        # 400 vector ALU instructions (v_*)
├── scalar.rs        # 93 scalar ALU instructions (s_* arithmetic/logic)
├── memory.rs        # 255 memory operations (buffer_*, ds_*, flat_*, global_*, s_load*, s_buffer_load*)
└── sys_control.rs   # 75 system/control instructions
```

#### sys_control.rs Organization

System and control instructions are organized with section comments:

```rust
// Control Flow Instructions
s_branch, s_call_b64, s_cbranch_*, s_endpgm*, s_sendmsg*, s_barrier, ...

// Execution Mask Management
s_and_*_saveexec_*, s_or_*_saveexec_*, s_*_wrexec_*, ...

// Quad/Wave Mode Control
s_wqm_*, s_quadmask_*

// System State Management
s_dcache_inv, s_icache_inv, s_denorm_mode, s_round_mode, s_delay_alu, ...

// Special Register Access
s_getreg_*, s_setreg_*

// Relative Addressing
s_movreld_*, s_movrels*

// Debug/Trace/Utility
s_ttracedata*, s_code_end, s_version, s_nop, s_clause
```

#### Categorization Logic

Instructions are categorized by pattern matching on normalized names:

- **Vector ALU**: `v_*` (excluding memory operations)
- **Scalar ALU**: `s_*` (excluding memory, control flow, and system operations)
- **Memory**: Keywords `load`/`store`/`atomic` OR prefixes `buffer_`, `ds_`, `flat_`, `global_`, `image_`, `tbuffer_`, `s_load*`, `s_buffer_load*`
- **System/Control**: Control flow patterns, exec mask manipulation, system state, special registers, etc.

#### Function Naming Convention

Handler functions use instruction names directly (no `op_` prefix):

```rust
// Generated in vector.rs
pub fn v_add_f32(ctx: &mut ExecContext, inst: &DecodedInst) -> ExecResult { ... }

// Generated in scalar.rs
pub fn s_and_b32(ctx: &mut ExecContext, inst: &DecodedInst) -> ExecResult { ... }

// Generated in memory.rs
pub fn buffer_load_format_x(ctx: &mut ExecContext, inst: &DecodedInst) -> ExecResult { ... }

// Generated in sys_control.rs
pub fn s_endpgm(ctx: &mut ExecContext, inst: &DecodedInst) -> ExecResult { ... }
```

#### Dispatch Table

Each arch module exports a sorted `OPS: &[(&str, Handler)]` lookup table:

```rust
// src/ops/base/mod.rs
pub static OPS: &[(&str, Handler)] = &[
  ("buffer_load_b32", memory::buffer_load_b32),
  ("s_and_b32", scalar::s_and_b32),
  ("s_endpgm", sys_control::s_endpgm),
  ("v_add_f32", vector::v_add_f32),
  // ... sorted by instruction name
];
```

Dispatcher uses binary search for fast lookups during execution.

---

## Instruction Pipeline

Instructions flow through a multi-stage validation pipeline from text to execution:

```
parse_file() → parse_instruction() → decode_instruction() → DecodedInst → Handler
    ↓               ↓                        ↓                   ↓
  raw text    ParsedInstruction    validate & convert    pre-validated ops
```

### ParsedInstruction (src/parse/instruction.rs)

Raw parsed instruction with string-based operands (output of text parsing):

```rust
pub struct ParsedInstruction {
    pub name: String,
    pub operands: Vec<Operand>,
}

pub enum Operand {
    Sgpr(u16), SgprRange(u16, u16),
    Vgpr(u16), VgprRange(u16, u16),
    SpecialReg(SpecialRegister),
    ImmU32(u32), ImmI32(i32), ImmF32(f32),
    Offset(u32), Flag(String),
    Negate(Box<Operand>), Abs(Box<Operand>),
}
```

Special instructions like `s_waitcnt` and `s_sendmsg` are pre-processed by the parser to pack named arguments into immediate values.

### DecodedInst (src/sim.rs)

Validated, execution-ready instruction with type-checked operands:

```rust
pub struct DecodedInst {
    pub name: String,
    pub def: &'static InstructionCommonDef,  // ISA metadata
    pub line_num: usize,                      // for error messages
    pub operands: Vec<DecodedOperand>,
}

pub enum DecodedOperand {
    Sgpr(u16), SgprRange(u16, u16),
    Vgpr(u16), VgprRange(u16, u16),
    SpecialReg(SpecialRegister),
    ImmU32(u32), ImmI32(i32), ImmF32(f32),
    Offset(u32), Flag(String),
    Negate(Box<DecodedOperand>), Abs(Box<DecodedOperand>),
}
```

`DecodedInst` carries the semantic guarantee that all operands have been validated against ISA specifications.

### Validation Rules (src/decode.rs)

The decoder validates each instruction against ISA metadata:

#### 1. Operand Count Matching
- Number of non-flag operands must match `def.args.len()`
- Flags are permissive and can appear anywhere

#### 2. Operand Type Validation

Each operand is validated against its corresponding `ArgSpec.kind`:

```
Operand::Sgpr/SgprRange    → ArgKind::Sgpr
Operand::Vgpr/VgprRange    → ArgKind::Vgpr
Operand::ImmU32/I32/F32    → ArgKind::Imm
Operand::Sgpr/Vgpr/Imm     → ArgKind::RegOrImm (accepts any)
Operand::SpecialReg        → ArgKind::Special
Operand::Offset            → ArgKind::Mem
Operand::Flag              → Always allowed
```

#### 3. Modifier Validation

Operand modifiers (`-reg`, `|reg|`, `-|reg|`) are validated based on instruction encoding:

**Supported Encodings:**
- `VOP3`: Full support for abs and neg modifiers
- `VOP3P`: Full support for abs and neg modifiers (packed operations)
- `SDWA`: Full support for abs and neg modifiers (CDNA only)
- `VINTERP`: Supports neg only (interpolation instructions)
- All other encodings: No modifier support

**Valid Examples:**
```
v_add_f32 v0, -v1, |v2|       ✓ VOP3 encoding supports modifiers
v_mul_f32 v0, -|v1|, v2       ✓ Combined modifiers (abs then negate)
v_add_f32 v0, -1.0, v1        ✓ Modifiers on immediates (data type compatible)
```

**Invalid Examples:**
```
s_mov_b32 s0, -s1             ✗ Scalar instructions don't support modifiers
v_add_f32 v0, -v[1:3], v2     ✗ Modifiers not allowed on ranges
v_add_u32 v0, |v1|, v2        ✗ Integer operations don't support modifiers
```

Validation occurs in two stages:
- **Parser validation**: Rejects modifiers on register ranges
- **Decoder validation**: Rejects modifiers on instructions that don't support them

#### 4. Special Operand Types

XML operand types map to ArgKind:
- `OPR_SSRC` (scalar source) → `RegOrImm` (accepts registers or immediates)
- `OPR_SDST` (scalar destination) → `Sgpr` (must be register)
- `OPR_WAITCNT` → `Imm` (parser converts to packed immediate)
- `OPR_SENDMSG` → `Imm` (parser converts to packed immediate)

### Instruction Lookup Process

When `parse_file()` encounters an instruction:

1. **Parse text** → `ParsedInstruction` via `parse_instruction()`
2. **Lookup definition**:
   - First check arch-specific ISA: `isa::{arch}::lookup_common_def(name)`
   - Fall back to base ISA: `isa::base::lookup_common_normalized(name)`
3. **Decode and validate** → `DecodedInst` via `decode_instruction()`
4. **Store** in `ProgramInfo.instructions: Vec<DecodedInst>`

### Error Handling

All validation errors include source line numbers and detailed messages:

```rust
pub enum DecodeError {
    UnknownInstruction(String, usize),
    OperandCountMismatch { expected, got, instruction, line },
    OperandTypeMismatch { expected, got, operand_index, instruction, line },
    InvalidOperand(String, usize),
}
```

**Example error messages:**
- `line 19: unknown instruction 's_mov_b33'`
- `line 21: instruction 's_waitcnt' expects 1 operands, got 2`
- `line 23: instruction 's_mov_b32' operand 2 expects RegOrImm, got Flag("omod")`

### Benefits of This Architecture

✅ **Fail Fast**: Invalid instructions caught during load, not execution
✅ **Simple Handlers**: Handlers read pre-validated `DecodedInst`, no need to re-check types
✅ **Centralized Validation**: Single source of truth in `decode.rs`
✅ **Better Errors**: Line numbers and detailed type mismatch messages
✅ **Performance**: Decode once, execute many times
✅ **Type Safety**: Rust type system enforces operand type correctness

### Special Cases

**Print Directives:**
Print pseudo-instructions are filtered out during parsing and not decoded. They will be handled separately via WaveState.

**Register Ranges:**
Some instructions use ranges like `s[0:3]`. `ArgSpec.width` indicates expected width in bits. Future validation may check that range size matches expected width.

**Flags and Modifiers:**
- Flags like `omod`, `clamp` are permissive and always allowed
- Operand modifiers (`-v0`, `|v0|`) are validated recursively

---

## Project Goals

### Core Simulator Features

**Wave-level debugging:**
- Step through RDNA instructions one at a time or across all waves
- Dump wave state (PC, SGPRs, VGPRs, EXEC, VCC, SCC) at any point
- Debug individual waves and threads with full visibility

**Execution model:**
- Wave-based execution (Wave32 and Wave64 support)
- Deterministic thread-to-lane mapping
- EXEC-mask based divergence modeling
- Single-threaded simulation (simplifies debugging, enables WASM target)

**Memory subsystem:**
- Global memory allocator mimicking real hardware
- LDS (Local Data Share) support
- Flat scratch implementation
- Async memory operations with `s_waitcnt`-style completion tracking

**Future enhancements:**
- Tensor core instruction emulator
- Delayed async reads with max throughput modeling
- CDNA support (Wave64-focused, MFMA instructions)

### Implementation Philosophy

**Not cycle-accurate:**
Focus on functional correctness (matching final results) rather than precise timing. This is partially educational, partially practical for running RDNA code without hardware.

**No trap handlers:**
Keep it simple - no exception handling for now. Still allocate all 128 scalar registers per architectural spec.

**Timing approach:**
Start with functional semantics (EXEC masks, waits, ordering). Add "timing-lite" later using:
- Radeon GPU Profiler (RGP) instruction timing data
- ROCm profiling counters (Omniperf)
- LLVM AMDGPU scheduling models

### User Experience

**REPL-based debugger:**
Straightforward CLI interface, potentially enhanced with `ratatui` for TUI.

**Multiple frontends:**
Project structured as library with pluggable frontends:
- CLI REPL
- TUI (terminal UI)
- WASM (browser-based)

**Output parity:**
Given RDNA input file, produce same output as real hardware (minus floating-point edge cases).

---

## Background: RDNA vs CDNA

### Execution Model Similarities

At the "inner loop" execution level, RDNA (client/graphics + compute) and CDNA (server/HPC) are fundamentally similar:

- Both execute kernels as **wavefronts** on SIMD lanes
- Both use per-wave state: PC, registers, EXEC mask
- Both use wave scheduler issuing instructions
- Divergence handled by EXEC mask gating

**Implication:** An RDNA simulator built around "waves as execution units" is a valid foundation that can extend to CDNA.

### Key Differences

**Wave size conventions:**
- RDNA: Commonly Wave32, also supports Wave64
- RDNA Wave64 executes as two Wave32 halves
- CDNA: Generally Wave64 in programming model

**Specialized instructions:**
- CDNA emphasizes matrix operations (MFMA-style)
- Different occupancy/resource ceilings
- CDNA-specific functional units

**Multi-chiplet packaging (MI350X):**
- Single accelerator package with multiple XCDs (compute dies) + I/O dies
- Connected via Infinity Fabric
- Supports partitioning
- Not "multi-GPU" in classic sense - shared device-level dispatch

**For simulation:**
Make wave size and execution quantum first-class configurable parameters. The inner wavefront execution abstraction remains the same.

### Performance & Timing Sources

**No official per-opcode latency tables:**
Unlike CPUs, AMD doesn't publish complete latency/throughput tables for GPU instructions. Memory latencies especially variable (cache state, contention, fabric, queueing).

**Practical timing sources:**

1. **Radeon GPU Profiler (RGP)**
   - Provides instruction timing using hardware support
   - Average issue latency per instruction
   - Useful for calibrating timing models

2. **ROCm profiling (rocprofiler-compute / Omniperf)**
   - Infer VMEM/SMEM latencies from counters
   - Kernel-level quantitative guidance
   - Not per-opcode constants

3. **LLVM AMDGPU backend**
   - Scheduling models and assumptions
   - Reasonable first approximation for ALU latencies
   - Not guaranteed to match hardware precisely

### Critical Correctness Work

**Not instruction cycles, but ISA semantics:**
- EXEC mask behavior for divergence
- SALU/VALU semantics
- Branches controlled by SCC/VCC
- Barriers and atomics
- Wait/fence mechanisms (`s_waitcnt` behavior)

**Pending memory model:**
Track outstanding VMEM/LGKM/etc. completions and enforce waits architecturally (even without simulating real time). This ensures kernels relying on waits behave correctly.

### Instruction Deduplication Strategy

**Two-layer architecture:**

1. **Architecture-specific decode/parse layer**
   - Keyed by: (arch target) + (encoding family) + (mnemonic/suffix) + operand grammar
   - Same mnemonic may have different encoding families (VOP2 vs VOP3 vs DPP vs SDWA)
   - Different modifiers/operand forms across generations

2. **Canonical semantic layer**
   - Deduplicated internal IR
   - Operation kind, type, exec-mask gating
   - Source/dest modifiers
   - Side effects (SCC/VCC/EXEC updates)
   - Memory effects

**Benefit:**
Supports "base set" of shared semantic ops while preserving generation-specific decoding and special cases.

### Summary

Start with RDNA as wavefront-execution simulator driven by raw GFX ISA. Make wave size and resources configurable. Focus on functional semantics (waits, ordering) to match results and enable debugging. Add timing later using profiler-derived data rather than hardcoded tables.
