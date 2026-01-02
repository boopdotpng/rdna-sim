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
