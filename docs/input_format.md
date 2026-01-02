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
- `vcc` - Vector condition code (wave32 uses low 32 bits; wave64 uses full 64)
- `vcc_lo`, `vcc_hi` - Vector condition code (low/high 32 bits)
- `exec` - Execution mask (wave32 uses low 32 bits; wave64 uses full 64)
- `exec_lo`, `exec_hi` - Execution mask (low/high 32 bits)
- `m0` - Memory dependency counter
- `null` - Discard destination
- `scc` - Scalar condition code

**Immediates:**
- Decimal, hexadecimal (`0x`), binary (`0b`), or floating-point
- Negative values allowed: `-42`, `-3.14`

**Modifiers:**

RDNA hardware provides exactly **two modifier bits** per VGPR source operand:
- One **NEG** bit (negation)
- One **ABS** bit (absolute value)

These are applied in a **fixed hardware order**: `value → abs → negate`

**Valid modifier combinations** (4 hardware states):
```
v0          # ABS=0, NEG=0
|v0|        # ABS=1, NEG=0
-v0         # ABS=0, NEG=1
-|v0|       # ABS=1, NEG=1 (abs then negate)
```

**Invalid combinations** (rejected by parser):
```
--v0        ❌ Double negation (only one NEG bit exists)
|-v0|       ❌ Abs after negate (wrong order; hardware applies abs before neg)
||v0||      ❌ Double absolute (only one ABS bit exists)
```

**Immediate modifier handling:**
- Negation allowed: `-42`, `-1.0`, `-0xFF` (folded at parse time)
- Absolute value NOT allowed: `|42|`, `|1.0|` (rejected by parser)

**Restrictions**:
  - Modifiers only allowed on single VGPRs (e.g., `v0`, `v1`)
  - NOT allowed on SGPR registers (e.g., `-s0`, `|s1|`)
  - NOT allowed on special registers (e.g., `-vcc_lo`, `|exec_hi|`, `-m0`)
  - NOT allowed on register ranges (e.g., `-v[1:3]`, `|s[0:1]|`)

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

Where `<arg>` can be `s[n]`, `s[lo:hi]`, `v[n]`, `vcc`, `vcc_lo`, `vcc_hi`, `exec`, `exec_lo`, `exec_hi`, or `scc`.

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
