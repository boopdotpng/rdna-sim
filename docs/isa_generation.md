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
pub fn v_add_f32(ctx: &mut Ctx) -> ExecResult { ... }

// Generated in scalar.rs
pub fn s_and_b32(ctx: &mut Ctx) -> ExecResult { ... }

// Generated in memory.rs
pub fn buffer_load_format_x(ctx: &mut Ctx) -> ExecResult { ... }

// Generated in sys_control.rs
pub fn s_endpgm(ctx: &mut Ctx) -> ExecResult { ... }
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

