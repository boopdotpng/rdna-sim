# rdna emu (only compute)

an emulator for RDNA (and eventually CDNA) GPUs. most of the focus right now is around RDNA 3.5 and RDNA 3 (these are very similar). the goal is to get correctnes first, and then focus on timing data, debugging, and profiling outputs like sqtt. 
 
- i still need to do a full pass through the code to 1) cut line count 2) make sure the tests are testing the right things and 3) add a lot of comments
- clean up and re-write the `docs/` folder (right now it's kind of ai slop)
- make one md file that contains all the documentation about rdna and make sure the emulator lines up with the spec 

the repo is probably still called `rdna-sim`. whoops

## instructions left to implement

- s_barrier
- s_delay_alu
- v_dual_* operations (done, tentative implementation is correct)
- buffer_atomic reads and writes to global memory and LDS-
- s_setreg and s_getreg
- accurate s_clause emulation
- s_waitcnt_depctr
- swizzle instructions (ds_swizzle_*)

## gpu simulation related items left to implement 

- accurate wave scheduling
- resource management 
- usage of the flat_scratch memory 
- race conditions when reading and writing memory 
- trap handler: `ttmp0..ttmp15`, `TBA` and `TMA`
- `vscnt` and related counters per wave
- accurate wmma simulation

## correctness tests / result comparision with kernels run on real amd gpus


## sqtt performance trace emulation 