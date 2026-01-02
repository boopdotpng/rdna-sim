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

