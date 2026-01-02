# RDNA Simulator — Agent Guide

## Documentation Structure

**This file contains only workflow and style guidance.**
Technical specifications are in `/docs/`:

- **`docs/input_format.md`** — Input file syntax (YAML header, instructions, operands, modifiers)
- **`docs/isa_generation.md`** — ISA XML parsing and code generation strategy
- **`docs/instruction_pipeline.md`** — Parsing → Decoding → Validation pipeline
- **`docs/handler_api.md`** — Instruction handler implementation guide
- **`docs/project_goals.md`** — Project goals and implementation philosophy
- **`docs/architecture.md`** — RDNA vs CDNA background, timing sources

**When to read docs:**
- Working on parsing/validation → read `instruction_pipeline.md`
- Implementing instruction handlers → read `handler_api.md`
- Modifying ISA generation → read `isa_generation.md`
- User asks about input file format → read `input_format.md`
- Questions about architecture/goals → read `project_goals.md` or `architecture.md`

---

## Style & Workflow

### Communication Style
- **Concise and direct** — short responses for CLI context
- **No emojis** unless explicitly requested
- **No unnecessary superlatives** or excessive praise
- **Objective technical accuracy** over validation
- **Professional tone** — prioritize facts over agreement

### File Operations
- **ALWAYS prefer editing** existing files over creating new ones
- **NEVER create markdown files** (README, docs, etc.) unless explicitly requested
- **Read before write** — always read a file before modifying it

### Tool Usage
- **Use specialized tools** over bash:
  - `Read` not `cat`/`head`/`tail`
  - `Edit` not `sed`/`awk`
  - `Write` not `echo >` or `cat <<EOF`
  - `Glob` not `find` or `ls`
  - `Grep` not `grep`/`rg`
- **Use Task tool** for multi-round exploration (not direct Grep/Glob)
- **Never guess URLs** unless helping with programming

### Code Quality
- **Avoid over-engineering** — only make requested changes
- **No premature abstractions** — three similar lines is better than a helper function
- **No extra features** — bug fix ≠ cleanup opportunity
- **No unnecessary comments/docstrings** on unchanged code
- **Security-aware** — watch for injection vulnerabilities (SQL, XSS, command injection)

### Git Workflow
- **Only commit when requested** by user
- **Never skip hooks** (no `--no-verify`, `--no-gpg-sign`)
- **Never force push** to main/master (warn user if requested)
- **Use heredoc for commit messages** to preserve formatting:
  ```bash
  git commit -m "$(cat <<'EOF'
  Commit message here.

  🤖 Generated with Claude Code
  Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>
  EOF
  )"
  ```

### Task Management
- **Use TodoWrite** frequently for multi-step tasks
- **Mark todos completed immediately** after finishing (don't batch)
- **One in_progress task** at a time
- **Break down complex tasks** into specific, actionable steps

---

## Testing Guidance

When adding or modifying tests:
- Place tests in `#[cfg(test)] mod tests` at end of file
- Use descriptive test names: `test_reject_modifier_on_sgpr` not `test_1`
- Test both valid and invalid cases
- Include error message validation for rejection tests

**Run tests:**
```bash
cargo test --lib                    # All tests
cargo test --lib module::tests      # Specific module
cargo test --lib test_name          # Specific test
```

---

## Project-Specific Conventions

### Modifier Parsing
RDNA has exactly **2 modifier bits** per VGPR (NEG, ABS), applied as `value → abs → negate`.

**Valid:** `v0`, `|v0|`, `-v0`, `-|v0|`
**Invalid:** `--v0`, `|-v0|`, `||v0||`, modifiers on SGPRs/ranges/special regs

See `docs/input_format.md` for full modifier specification.

### Special Registers
In RDNA: `vcc_lo`, `vcc_hi`, `exec_lo`, `exec_hi`, `m0`, `scc`, `null`
**Note:** `vcc` and `exec` do NOT exist (only `_lo`/`_hi` variants)

### Error Handling
- Parser errors: operand syntax, modifier restrictions
- Decoder errors: instruction validation, operand type checking
- Include line numbers in all error messages

---

**For detailed technical specifications, always refer to `/docs/` files.**

**do not care about maintaining backwards compatibility** 
**any new functionality you write must be tested or be covered by existing tests.**