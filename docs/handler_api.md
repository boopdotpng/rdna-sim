## Instruction Handler API

### The `Ctx` Struct

All instruction handlers receive a single unified context parameter that bundles wave state, memory, and the current instruction:

```rust
pub struct Ctx<'a> {
    pub wave: &'a mut WaveState,
    pub lds: &'a mut LDS,
    pub global_mem: &'a mut GlobalAlloc,
    pub inst: &'a DecodedInst,
}
```

Handler signature:
```rust
pub fn s_load_b64(ctx: &mut Ctx) -> ExecResult { ... }
```

### Helper Methods

Minimal helpers are provided for common dest handling and source access:
```rust
ctx.dst_sgpr()           // Operand 0 as a single SGPR
ctx.dst_sgpr_range()     // Operand 0 as an SGPR range (start index)
ctx.dst_vgpr()           // Operand 0 as a single VGPR
ctx.dst_vgpr_range()     // Operand 0 as a VGPR range (start index)
ctx.src(idx)             // Borrow operand at index (use match in handler)
```

All other state is accessed directly:
```rust
ctx.wave.*               // SGPR/VGPR/EXEC access
ctx.lds.*                // LDS access
ctx.global_mem.*         // Global memory (MemoryOps)
```

**Modifier handling:**
- Modifiers are only allowed on single VGPRs: `-v0`, `|v1|`, `-|v2|`
- They are represented as `DecodedOperand::Negate` and `DecodedOperand::Abs` wrappers
- Immediate negation (`-42`, `-1.0`) is folded at **parse time** into `ImmI32(-42)` / `ImmF32(-1.0)`, not wrapped in `Negate`
- SGPRs, special registers, and register ranges never accept modifiers (rejected at parse time)

---

## Dispatch & Generated Tables

Instruction execution is routed through two generated tables per ISA:

- `OPS`: manual handler stubs for non-typed instructions
- `TYPED_OPS`: typed variants that share implementations across data types

Typed handlers live in:
- `src/ops/typed_v_ops.rs` for vector ops (`v_*`)
- `src/ops/typed_s_ops.rs` for scalar ops (`s_*`)
- `src/ops/typed_mem_ops.rs` for memory/lds ops (`ds_*`, `buffer_*`, `flat_*`, `global_*`, `image_*`)

---

## Adding New Handlers

### Typed variants
If an instruction has multiple datatype variants (e.g. `v_add_f16`, `v_add_f32`), it is routed through `TYPED_OPS`.
Add new typed behavior by extending the mappings in `scripts/gen_isa.py` and implementing the handler in the appropriate `typed_*_ops` file.

### Manual handlers
Instructions that do not have multiple datatype variants are emitted as manual stubs in the `src/ops/*/manual_*_ops.rs` files.
To implement one, edit its stub directly. Regenerating ops will recreate these stubs, so finish manual edits in one pass before re-running `--write-ops`.

### Example Implementations

**Scalar Memory Load (`s_load_b64`):**
```rust
pub fn s_load_b64(ctx: &mut Ctx) -> ExecResult {
    let dst = ctx.dst_sgpr_range();
    let base = match ctx.src(1) {
        DecodedOperand::SgprRange(start, _) => ctx.wave.read_sgpr_pair(*start as usize),
        _ => panic!("expected SgprRange for base"),
    };
    let offset = match ctx.src(2) {
        DecodedOperand::Sgpr(n) => ctx.wave.read_sgpr(*n as usize) as u64,
        DecodedOperand::ImmU32(v) => *v as u64,
        DecodedOperand::ImmI32(v) => *v as u64,
        _ => panic!("expected Sgpr or Imm for offset"),
    };

    let addr = base + offset;
    let value = ctx.global_mem.read_u64(addr).unwrap();
    ctx.wave.write_sgpr_pair(dst, value);
    Ok(())
}
```

**Vector ALU (`v_add_f32`):**
```rust
pub fn v_add_f32(ctx: &mut Ctx) -> ExecResult {
    for lane in 0..ctx.wave.wave_lanes() {
        if !ctx.wave.is_lane_active(lane) { continue; }

        // Match on ctx.src(1)/ctx.src(2) for Vgpr/Sgpr/Imm and apply modifiers as needed.
        let _ = lane;
    }
    Ok(())
}
```

**Benefits:**
- **Explicit control**: Handlers choose how to handle each operand path
- **No error handling**: Operands pre-validated by decoder
- **Type-safe**: Rust enforces correctness at compile time

---
