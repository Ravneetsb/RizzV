# Rizz-V Project Brief

## Overview

Rizz-V is a Rust-based RISC-V-inspired emulator with an accompanying execution visualization workflow. The project takes assembly source, parses it with a formal grammar, lowers it into typed instruction structures, resolves labels, runs the program through a small CPU and memory model, emits a step-by-step execution trace, performs static control-flow analysis, and exposes the results through both a CLI and a local browser UI.

At an interview level, the project is best described as a systems programming and tooling project: it combines parsing, instruction representation, emulation, memory safety checks, static analysis, JSON artifact generation, and frontend visualization into one coherent pipeline.

## What The Project Does

Rizz-V accepts assembly programs that use a scoped subset of RISC-V-style instructions and pseudo-instructions. It can run programs from the command line, write JSON artifacts for assembly, execution trace, control-flow analysis, and final result decoding, or serve a local web interface where users can upload assembly, configure register inputs, run the emulator, and inspect execution visually.

The core execution pipeline is:

```text
assembly source
-> pest parse tree
-> Assembler
-> resolved typed instructions
-> control-flow analysis
-> Executor
-> trace and final result
-> JSON artifacts or browser visualization
```

This makes the project useful for understanding how assembly-level code executes over time, especially how registers, branches, jumps, stack memory, and function calls interact.

## Architecture And Design

### Parser And Assembler

The parser is built with `pest` and a custom grammar in `rizz-v/src/grammar.pest`. The grammar recognizes directives, labels, comments, registers, immediates, branch operands, memory operands, and common assembly-style instruction formats.

The assembler converts the parse tree into strongly typed Rust data structures rather than storing raw strings. Instructions are represented as variants for R-type, I-type, load, store, branch, jump, and nil instructions. This design makes execution logic explicit and type-driven: the executor can match on instruction variants and avoid repeated parsing or string dispatch during runtime.

The assembler also handles label collection and label resolution. During the first pass, labels are mapped to program-counter addresses. During resolution, branch and direct jump targets are rewritten from label names into concrete addresses. The `.global` directive is used to determine the entry point; when multiple globals exist, the lowest-address global label is selected.

Pseudo-instructions are lowered into simpler internal forms. Examples include:

- `li rd, imm` lowered to `addi rd, zero, imm`
- `mv rd, rs` lowered to `addi rd, rs, 0`
- `ret` lowered to an indirect jump through `ra`
- `j label` lowered to `jal zero, label`
- zero-compare branch forms like `beqz` lowered to regular branch instructions against `zero`

This lowering step is an important design choice because it keeps the executor smaller: runtime execution only needs to implement the canonical internal instruction forms.

### CPU And Register Model

The CPU model tracks a byte-addressed program counter and a 32-entry integer register file. Registers are represented by a typed enum rather than arbitrary numeric indexes, which makes instruction construction and execution clearer.

The register file enforces RISC-V zero-register semantics: writes to `zero` are ignored. This behavior is centralized in the register file instead of scattered throughout instruction execution, so every instruction automatically respects the invariant.

The executor initializes `sp` to the top of the stack window and optionally injects user-provided input registers before step zero. Register input parsing rejects writes to `zero`, which prevents invalid user configuration from bypassing the architectural rule.

### Memory Model

Memory is modeled as a sparse byte map backed by a `BTreeMap<u64, u8>`. Instead of emulating a full address space, the project intentionally provides a bounded stack window from `0x1000` to `0x2000`, with the initial stack pointer near the top of that region.

The memory layer supports byte, halfword, word, and doubleword loads and stores. It validates:

- Address bounds
- Address overflow
- Alignment for 16-bit, 32-bit, and 64-bit accesses
- Signed and unsigned interpretation for supported load widths

This design keeps the emulator approachable while still modeling the memory safety issues that matter for stack-based assembly programs. It also makes memory failures explicit and testable, such as rejecting misaligned word stores.

### Executor And Trace Generation

The executor runs one instruction at a time from the assembled program. For each step, it records a `State` object containing:

- Step number
- Current and next program counter
- Full register file snapshot
- Executed instruction
- Branch decision, when applicable
- Memory load/store events, when applicable
- Termination status

This trace-first design is central to the project. Rather than only computing the final register state, the emulator preserves the full execution history. That makes it possible to drive the visualization UI, debug assembly programs, and explain control flow during interviews.

The executor includes a max-step guard, defaulting to 10,000 steps, to prevent accidental infinite loops from running forever. Control-flow termination is modeled through `ret`: an indirect jump through `ra` terminates the run when it jumps to address `0` with destination register `zero`.

### Static Control-Flow Analysis

The analysis module constructs a control-flow graph from the resolved instruction stream. It identifies basic block leaders from entry points, branch targets, and fallthrough addresses, then groups instruction addresses into basic blocks.

Edges are classified as:

- Fallthrough
- Branch taken
- Branch not taken
- Jump

Loop detection is implemented by depth-first search over the block graph. Back edges are converted into loop summaries containing the loop header, back-edge source, and member blocks. This gives the visualization and generated JSON a higher-level view of the program beyond raw instruction stepping.

### CLI, HTTP Service, And Visualization

The command-line interface supports running an input `.s` file and writing four output artifacts:

- Assembled program JSON
- Execution trace JSON
- Control-flow analysis JSON
- Final result JSON

The CLI also supports register initialization, max-step configuration, scalar result decoding, and array result decoding from memory.

The local server mode binds to `127.0.0.1` and serves the bundled `viz3.html` UI. The browser UI posts source code and run configuration to `/api/run`, then displays the returned assembly, trace, analysis, and interpreted result. The server is intentionally small and implemented with the Rust standard library TCP types, which keeps deployment simple and makes the request/response flow transparent.

The visualization is useful because it turns the emulator from a backend-only system into an interactive tool. Users can inspect register changes, memory events, branch behavior, control-flow structure, and decoded return values without manually reading JSON.

## Supported Features

The implemented instruction set focuses on a practical subset of integer and control-flow behavior:

- R-type operations: `add`, `sub`, `div`, `mul`, `sll`
- I-type operation: `addi`
- Loads: `lb`, `lbu`, `lh`, `lhu`, `lw`, `ld`
- Stores: `sb`, `sh`, `sw`, `sd`
- Branches: `beq`, `bne`, `blt`, `bge`, `bltu`, `bgeu`
- Jumps and calls: `jal`, `ret`, and direct jump pseudo-instruction support
- Pseudo-instructions: `li`, `mv`, `ret`, selected branch aliases, and zero-compare branches
- Directives: `.global`
- Register input injection through CLI flags or the HTTP API
- Scalar result decoding from a selected register
- Array result decoding from memory using pointer and length registers
- JSON serialization for assembled code, traces, control-flow analysis, and results

## Testing And Reliability

The codebase includes unit tests around the core behavioral risks of an emulator:

- Backward branch label resolution
- Loop execution and loop-header detection
- `jal` and `ret` call behavior
- Nested call stack save and restore behavior
- Signed and unsigned load semantics
- Misaligned memory access rejection
- Unknown label rejection
- Register input parsing
- Zero-register assignment rejection
- Input register injection into initial state and trace metadata
- Final scalar result decoding
- Array result decoding from stack memory
- Array decode error reporting without failing the whole run

The most important reliability decision is that parsing, assembly, execution, memory access, and result interpretation return explicit errors rather than silently producing partial output. The max-step limit also protects both CLI and server execution from non-terminating programs.

## Design Tradeoffs

Rizz-V intentionally favors clarity and inspectability over full ISA completeness. It implements enough of a RISC-V-like subset to demonstrate function calls, stack use, arithmetic, branches, loads, stores, and loops, but it is not a full RISC-V virtual machine.

The stack-only memory model is another deliberate simplification. It avoids the complexity of heap, global data, and full process memory while preserving the interesting parts of assembly execution needed for stack frames, local values, and array result decoding.

The HTTP server is small and single-process. It is appropriate for a local visualization tool, but it is not designed as a production web service. That tradeoff keeps the project dependency-light and makes the backend-to-frontend contract easy to inspect.

The typed instruction model adds some upfront modeling work, but it pays off by making the executor and analyzer easier to reason about. Instead of repeatedly parsing operands, the project does validation and lowering once in the assembler.

## Interview Talking Points

Strong interview angles for this project include:

- Built a complete source-to-execution pipeline rather than only an emulator loop.
- Used a formal parser to avoid brittle manual string parsing.
- Modeled instructions with Rust enums and structs to make invalid runtime states harder to represent.
- Separated parsing, assembly, analysis, execution, memory, and presentation concerns into independent modules.
- Implemented trace generation as a first-class output, enabling debugging and visualization.
- Added static control-flow analysis to identify basic blocks, edges, and loops from resolved instructions.
- Preserved architectural invariants such as the zero register and aligned memory access in centralized components.
- Designed JSON artifacts so the same backend pipeline can support both CLI workflows and a browser UI.
- Included guardrails for invalid labels, bad registers, invalid memory access, and infinite execution.

## Limitations And Future Work

The project currently implements a focused subset of RISC-V-style integer instructions. Future work could expand opcode coverage, add more immediate forms, support data sections, model heap/global memory, or add richer system-call behavior.

The visualization could also be extended with timeline scrubbing, graph rendering for the control-flow analysis, diff views between consecutive register states, and better source-to-instruction mapping.

On the backend, future improvements could include a more complete HTTP framework, concurrent request handling, structured API error codes, benchmark tests, and property-based testing for instruction semantics.

## Resume Entry Starter

- Built Rizz-V, a Rust-based RISC-V-inspired emulator and visualization tool that parses assembly with `pest`, lowers it into typed instruction models, executes programs step-by-step, and emits JSON traces for debugging and analysis.
- Implemented core systems components including label resolution, register-file semantics, bounded stack memory with alignment checks, function-call execution, branch handling, max-step protection, and scalar/array result decoding.
- Added static control-flow analysis and a local browser UI to inspect execution traces, memory events, branch decisions, basic blocks, loop summaries, and final program outputs.

