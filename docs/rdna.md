# rdna documentation 
This was taken from the RDNA3.5 ISA pdf. 

## states we need to keep track of 

### per program 
- Global memory (allocate 512mb box to start)
- Program counter (PC) points to first instruction when wave is created 

| State        | Description                                                         | Width / Range    |
| ------------ | ------------------------------------------------------------------- | ---------------- |
| TBA          | trap base address                                                   | 48-bit           |
| TMA          | Trap memory address                                                 | 48-bit           |

**a note about branch jumps** 
- branches jump to pc_of_the_instruction_after_the_branch + offset*4
- get_pc and swap_pc are relative to the next instruction, not the current one.
- all prior instructions have been issued but may or may not have completed execution

### state per wave 

| State        | Description                                                         | Width / Range    |
| ------------ | ------------------------------------------------------------------- | ---------------- |
| SGPRs        | scalar general purpose registers                                    | s0–s105          |
| VGPRs        | vector general purpose registers                                    | v0–v255, per-lane u32 |
| LDS          | do we need to emulate cache? scratch ram                            | —                |
| EXEC         | top half not used in wave32                                         | 64-bit           |
| EXECZ        | exec is zero                                                        | 1-bit            |
| VCC          | vector condition code                                               | 64-bit           |
| VCCZ         | vcc is zero                                                         | 1-bit            |
| SCC          | scalar condition code                                               | 1-bit            |
| Flat_scratch | base address for scratch memory used this wave (overflow registers) | 48-bit           |
| M0           | misc reg                                                            | 32-bit           |
| TRAPSTS      | trap status                                                         | 32-bit           |
| TTMP0-TTMP15 | trap temporary SGPRs                                                | 32-bit           |
| VMcnt        | vmem load and sample instructions issued but not yet completed      | 6-bit            |
| VScnt        | vmem store instructions...                                          | 6-bit            |
| EXPcnt       | export/gds instructions (do we need this)                           | 3-bit            |
| LGKMcnt      | lds, gds, constant and message count                                | 6-bit            |

**PC**
Program counter: Next shader instruction to execute. Read/write only via scalar control flow instructions and indirectly using branch. 2 LSBs are forced to zero. (what does that mean?)

**EXECute Mask**

Controls which threads in the vector are executed. 1=execute, 0=do not execute. Exec can be read/written via scalar instructions.
Can be written as a result of vector-alu compare. 

Exec affects: vector-alu, vector-memory, LDS, GDS, and export instructions. No effect on scalar execution / branches. 

Wave64 uses all 64 bits, wave32 only uses 31:0. In wave32, exec_hi is always 0.

*Instruction skipping (exec=0):*
**todo: this makes no sense right now**

**SGPRs**

106 normal SGPRs. vcc_hi and vcc_low are technically stored in SGPR 106 and 107. 

Alignment for SGPRs: 
- any time 64-bit data is used
- scalar memory reads when the address-base comes from an SGPR pair (loading in arguments, i guess) 

Other notes: 
- Writes to an out-of-range SGPR are ignored

**VCC**
Vector condition code written by V_CMP and integer vector add/sub instructions. vcc is read by many instructions.
Wave64 uses all 64 bits, wave32 only uses 31:0. In wave32, vcc_hi is always 0.
named SGPR pair, subject to same dependency checks (?) as toher SGPRs. 


**VGPRs**
Each VGPR index holds one u32 per lane. Model as a 2D file:

VGPR[reg][lane]

reg in 0..VGPR_COUNT-1, lane in 0..wave_size-1 (32 or 64). Writes are masked by EXEC: lane writes only happen when EXEC[lane] is 1.

## data types 
- b32 (binary untyped 32-bit), this is not really used 
- b64
- f16
- f32
- f64. 
- bf16
- i8
- i16
- i32
- i64
- u16
- u32
- u64

## syntax / parsing

Take hand-written RDNA and run it. To inject data into global memory (for the kernel to run on), we'll have a section at the top of the file: 
---
arg_1: i64 = []...  or a scalar
arg_2: 

**copy these into global memory and then fill s[0:1] with a 64-bit pointer to the argument table.**

---

*instructions are just one opcode and N arguments afterwards. simple parsing.*
see `example.rdna` at repo root. 


---
printing section. after a program runs, you place your results back in global memory for the computer to read back. list the addresses you want printed here and what datatype.

**todo: determine format of this?**
0xfff : i64 
---

We have to ignore some instructions, because this is not a timing simulator (maybe in the future)? So `s_delay_alu` and friends will be completely ignored. 
