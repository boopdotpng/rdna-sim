## parsing the input file
Take handwritten RDNA and run it. The input file has a header block and an instruction block.

### header block
Header lives between two `---` lines. It is YAML-ish and only needs a simple parser.

Example:
---
arg_a: f32[16] = arange(16)
arg_b: i32 = 3
out_y: f32[4,4]
local = (N, 1, 1)
global = (N, 1, 1)
wave = 32
---

Rules:
- `name: type` declares a scalar. `name: type[shape]` declares an array.
- `shape` can be `n` or `r,c,...` in brackets. Shapes are flattened in row-major order.
- `out_*` variables are regular arrays allocated in global memory and printed after the kernel finishes.
- Arrays must have a size in the header, even if they are outputs only.
- Number parsing accepts decimal, `0x` hex, and `0b` binary. Floats use standard decimal form.
- Initializers (`=`) are optional. If present, they must supply enough values for the flattened size.
- `arange(n)` or `arange(start, end[, step])` produces a 1D sequence. If a shape is provided, fill it row-major.
- `matrix(r, c[, start[, step]])` is shorthand for a row-major matrix starting at `start` (default 0).
- `local = (x, y, z)` is threads per workgroup (CUDA block equivalent).
- `global = (x, y, z)` is workgroups (CUDA grid equivalent).
- `wave = 32` or `wave = 64` picks the wave size for the launch.

Arguments should be copied into global memory and a 64-bit kernarg pointer should be placed in SGPRs.
Follow the PAL/ABI layout as closely as possible. If unsure, default to `s[0:1]` holding the pointer.

### instruction block
Instructions are one opcode followed by N arguments. Keep parsing simple: read the first word, then split args by `,`.
See `example.rdna` at repo root once the syntax is locked in.

Instructions with parentheses must parse the `(...)` arguments, like:
- `s_sendmsg sendmsg(...)`
- `s_waitcnt lgkmcnt(0) vmcnt(0)`

#### print queue
Add a pseudo-instruction to enqueue debug prints. These are collected and emitted after the kernel finishes.
Prints are not masked by EXEC; they always run, but each entry includes whether the selected lane is active.

Syntax:
- `print [wave=<id>,] [thread=<id>|thread=all,] <arg>[, <arg>...]`
- `<arg>` can be `s[n]`, `s[lo:hi]`, `v[n]`, `exec`, `vcc`, or `scc`.

Behavior:
- `wave` is a filter. If set and it does not match the current wave id, do nothing.
- `thread` picks which thread to read VGPRs from. Default is `thread=0`. `thread=all` prints one entry per thread.
- Each print entry includes: wave id, thread id, `active` (EXEC bit for that thread), and the value(s).

We have to ignore some instructions, because this is not a timing simulator (maybe in the future)? So `s_delay_alu` and friends will be completely ignored. 
Maybe we can add some artifical delay that messes up kernels that don't wait. how to do this? 

## data 
in `data/` there are xmls for each ISA. gen_isa.py has a script that parses instructions out of one isa file into a generated rust file. is this the best approach? can we do better? we also need to de-duplicate instructions, and maybe have a "base" set of instructions that are the same across all ISAs then go beyond that. and then for intstructions that are unknown in the XML, we can handle them as they come up. 

## ultimate project goals

- debug individual waves and threads 
- dump Wave state in every wave 
- step through rdna instructions one at a time or across all wave 
- spin up CPU threads to handle large waves (is this necessary? can we just model this all single-threaded to make it more simple?)
- global memory allocator that mimics the real thing, with delayed async reads and a max throughput? 
- LDS flat_scratch implementation 
- Tensor Core instruction emulator 
- no trap handlers for now. i dont want to deal with it. still allocate all 128 registers. 
- wave32 and wave64 support so we can transition to CDNA simulation later. i'm mostly concerned with the wave level simulation, because this is very similar on all AMD gpus. the thing scheduling the waves and memory is usually teh difference between RdNA/CDNA. 

### final result 
you put in an RDNA file, and you get the exact same output as the real hardware (minus weird floating point errors). this isnt 100% correct, partially educational and partially so that people can run and learn rdna without having hardware. this will also eventually compile to WASM to run in the browser. so single-threaded is probably fine for small launches. 

REPL-based debugger to begin, with a straightforward interface, or we use rust-tui or something to make it look nice? 


then we'll do WASM. the project is currently structured as a library with multiple frontends so we can plug in a TUI, web application, or compile to WASM. 


## misc. rdna/cdna docs/project references 
Here’s a single-message summary you can drop into your project docs.

---

## Summary: RDNA vs CDNA execution mechanics, timing sources, and instruction dedup strategy for an ISA-level simulator

We discussed whether AMD CDNA (server/HPC) GPUs execute work fundamentally differently from RDNA (client/graphics + compute). At the “inner loop” execution model level, both families execute kernels as wavefronts on SIMD lanes with per-wave state (PC, registers, EXEC mask, etc.) and a wave scheduler issuing instructions. So an RDNA simulator built around “waves are the unit of execution and scheduling” is a valid foundation that can later be extended toward CDNA.

Important differences that matter for simulator dynamics (even if you ignore cycle-accurate timing) include wave size conventions, occupancy/resource ceilings, and specialized functional units (especially matrix/MFMA-style ops in CDNA). RDNA commonly uses Wave32 (and also supports Wave64), and RDNA materials describe Wave64 execution as behaving like two Wave32 halves. CDNA is generally Wave64 in its programming model, and CDNA documentation emphasizes per-wavefront matrix operations as a key compute feature. These differences argue for making wave size and “execution quantum” first-class parameters in the simulator.

We also corrected a packaging assumption about MI350X/Mi350-series: rather than behaving like multiple separate GPUs each with their own independent front ends, it’s a single accelerator package built from multiple chiplets (XCD compute dies plus I/O dies) connected via Infinity Fabric, with support for partitioning. For simulation, that suggests an “outer” device distribution model with replicated compute partitions (chiplets) under a global device-level dispatch concept, rather than treating it as multi-GPU in the classic sense. The “outer” dispatch and memory fabric effects change, but the “inner” wavefront execution abstraction still applies.

On performance/timing: there is no single official, complete per-opcode latency/throughput table from AMD that you can reliably import for RDNA/CDNA in the way CPU folks might expect. Memory operation latencies are especially variable (cache/TLB state, contention, fabric, queueing), so a fixed cycle count isn’t stable. However, there are practical sources of “rough instruction timing” and latency proxies:

* Radeon GPU Profiler (RGP) can provide instruction timing (average issue latency per instruction) using hardware support on AMD GPUs; this is useful for calibrating a timing-lite model later.
* ROCm profiling/counters (rocprofiler-compute / Omniperf) can be used to infer average VMEM/SMEM-type latencies and stalls from counters, providing kernel-level quantitative guidance even if not per-opcode constants.
* LLVM’s AMDGPU backend contains scheduling models and assumptions that can serve as a reasonable first approximation for ALU-type latencies/hazards, though they are not guaranteed to match hardware precisely.

Given your stated goal (not cycle-accurate; more “functional-exact” final results plus wave/register debugging), the highest-value correctness work is not instruction cycles but ISA semantics: EXEC mask behavior for divergence, SALU/VALU semantics, branches controlled by SCC/VCC, barriers and atomics, and especially the wait/fence mechanisms that govern when async memory results become visible (e.g., `s_waitcnt`-style behavior). A workable approach is to implement an architectural “pending memory” model (track outstanding VMEM/LGKM/etc. completions and enforce waits) even without simulating real time, so that kernels relying on waits for correctness behave the same.

For the simulator structure and debugging UX:

* Treat “wave state” as the fundamental object: per-wave PC, SGPRs, special scalar regs (EXEC, VCC, SCC), and per-lane VGPRs.
* Use a deterministic thread-to-lane mapping (local linear thread id → wave id + lane id) so the debugger consistently shows “lane N corresponds to work-item N within that wave.”
* Implement divergence by modeling EXEC-mask gating rather than higher-level SIMT constructs; correct EXEC manipulation and branching yields realistic stepping and VGPR/SGPR views.
* You plan to start with RDNA compute kernels (no graphics) using Wave32 with an option for Wave64 on RDNA3/3.5/4, and later expand to CDNA.

On instruction parsing and deduplication across ISAs/generations: deduplicating purely by mnemonic string is unsafe. The same conceptual op may appear in different encoding families (e.g., VOP2 vs VOP3 vs DPP vs SDWA variants) and may allow different modifiers/operand forms across generations, which affects parsing and sometimes semantics. The recommended architecture is a two-layer table:

1. An architecture- and encoding-specific decode/parse layer keyed by (arch target) + (encoding family) + (mnemonic and forced suffix/variant) + operand/modifier grammar.
2. A deduplicated semantic layer that maps parsed instructions into a canonical internal IR (operation kind, type, exec-mask gating, source/dest modifiers, side effects like SCC/VCC/EXEC updates, and memory effects). This supports a “base set” of shared semantic ops across ISAs while preserving generation-specific decoding and special cases (especially for memory, waits, atomics, and anything touching EXEC/VCC/SCC).

Overall: start with RDNA as a wavefront-execution simulator driven by raw GFX ISA disassembly. Make wave size and resource ceilings configurable. Focus first on functional semantics (including waits/ordering) to match final results and enable rich per-wave debugging. Add “timing-lite” later using profiler-derived instruction timing and counter-based latency inference rather than trying to hardcode a complete cycle table.

---

## ISA generation + encoding plan
- Parse RDNA3, RDNA3.5, and RDNA4 XMLs together and build a strict common set of instructions.
- Define "common" as a full signature match: name + ordered operand specs + encoding list.
- Emit a base table of shared instruction defs, then per-arch tables that either reference the base entry or an arch-specific entry.
- Codegen prints instruction counts (base + per-arch totals + arch-specific counts) after generation.
- Keep encodings as parsing metadata (operand grammar + modifiers), not as bit-level layout.
- When decoding disassembly, use `(mnemonic, encoding suffix)` to choose the operand grammar and supported modifiers.
- Treat DPP/SDWA and other lane/sub-dword encodings as semantic features, not just metadata.

## Instruction dispatch + handlers
- Codegen emits `src/ops/base.rs` and `src/ops/{rdna3,rdna35,rdna4}.rs` with one stub handler per instruction.
- Each ops module exports a sorted `OPS: &[(&str, Handler)]` table keyed by instruction name.
- Handlers share a signature that takes `ExecContext` (wave + program state) and a decoded instruction.
- Dispatch does binary search over arch ops first, then base ops, and calls the handler.
