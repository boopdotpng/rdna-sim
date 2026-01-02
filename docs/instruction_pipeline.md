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

**Hardware Modifier Model:**

RDNA provides exactly **two modifier bits** per VGPR source operand, applied in hardware order:
```
value → abs → negate
```

This creates **4 valid states** corresponding to the only meaningful combinations:
- `v0` (ABS=0, NEG=0)
- `|v0|` (ABS=1, NEG=0)
- `-v0` (ABS=0, NEG=1)
- `-|v0|` (ABS=1, NEG=1) — the **only** valid combined form

Invalid combinations are rejected because they attempt to:
- Use non-existent modifier bits: `--v0` (no second NEG bit), `||v0||` (no second ABS bit)
- Apply modifiers in wrong order: `|-v0|` (would require abs after neg, but hardware does abs→neg)

**Parse-time validation:**
- Modifiers only allowed on single VGPRs (e.g., `v0`, `v1`)
- Rejected on SGPRs: `-s0`, `|s1|`, `-|s2|`
- Rejected on special registers: `-vcc_lo`, `|exec_hi|`, `-m0`, `|scc|`
- Rejected on register ranges: `-v[1:3]`, `|s[0:1]|`
- Invalid combinations rejected: `--v0`, `|-v0|`, `||v0||`
- Absolute value rejected on immediates: `|42|`, `|1.0|`, `-|0xFF|`
- Negation on immediates IS allowed and handled at parse time: `-42` → `ImmI32(-42)`, `-1.0` → `ImmF32(-1.0)`

**Decode-time validation (instruction encoding support):**
- `VOP3`: Full support for abs and neg modifiers
- `VOP3P`: Full support for abs and neg modifiers (packed operations)
- `SDWA`: Full support for abs and neg modifiers (CDNA only)
- `VINTERP`: Supports neg only (interpolation instructions)
- All other encodings: No modifier support

**Valid Examples:**
```
v_add_f32 v0, -v1, |v2|       ✓ VOP3 encoding supports modifiers, applied to VGPRs
v_mul_f32 v0, -|v1|, v2       ✓ Combined modifiers (abs then negate)
v_add_f32 v0, -1.0, v1        ✓ Immediate negation handled at parse time
```

**Invalid Examples:**
```
# Parse-time rejections (operand type / invalid combinations)
s_mov_b32 s0, -s1             ✗ Modifiers not allowed on SGPRs
v_add_f32 v0, -v[1:3], v2     ✗ Modifiers not allowed on ranges
v_add_f32 v0, |vcc_lo|, v1    ✗ Modifiers not allowed on special registers
v_add_f32 v0, |42|, v1        ✗ Absolute value not allowed on immediates
v_add_f32 v0, --v1, v2        ✗ Double negation (only one NEG bit exists)
v_add_f32 v0, |-v1|, v2       ✗ Abs after neg (hardware does abs→neg, not neg→abs)
v_add_f32 v0, ||v1||, v2      ✗ Double absolute (only one ABS bit exists)

# Decode-time rejections (instruction encoding support)
s_mov_b32 s0, -v1             ✗ Scalar instruction doesn't support modifiers
v_add_u32 v0, |v1|, v2        ✗ Integer operation doesn't support abs modifier
```

**Immediate modifier folding:**
- Negative immediates (`-42`, `-1.0`, `-0xFF`) are parsed and folded at **parse time**
- The parser produces `ImmI32(-42)` directly, not `Negate(ImmI32(42))`
- Instruction handlers receive immediates with their sign already applied

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
- Flags like `omod`, `clamp`, `glc` are permissive and always allowed
- Operand modifiers (`-v0`, `|v0|`, `-|v0|`) are validated at parse time (operand type restrictions) and decode time (instruction encoding support)
- Modifiers are only allowed on single VGPRs, not on SGPRs, special registers, or ranges
- Immediate negation (`-42`) is handled at parse time and produces a negative immediate directly

---

