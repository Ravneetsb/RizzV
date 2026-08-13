use pest::Parser;
use pest_derive::Parser;
use serde::Serialize;
use std::str::FromStr;

pub mod analysis;
pub mod assembler;
pub mod cpu;
pub mod executor;
pub mod instruction;
pub mod memory;
pub mod reg;
pub mod state;

use analysis::{ControlFlowAnalysis, analyze};
use assembler::Assembler;
use executor::{Executor, RunConfig};
use memory::Memory;
use reg::Register;
use state::RegisterValue;

/// Parser for the Rizz assembly grammar.
#[derive(Parser)]
#[grammar = "grammar.pest"]
pub struct RizzParser;

/// Outputs captured from a full source-to-result run.
#[derive(Debug, Serialize)]
pub struct RunArtifacts {
    /// JSON serialization of the assembled program.
    pub assembly: serde_json::Value,
    /// JSON execution trace emitted by the executor.
    pub trace: serde_json::Value,
    /// Static control-flow summary derived from the assembled program.
    pub analysis: ControlFlowAnalysis,
    /// Final result decoded from the executor state.
    pub result: FinalResult,
}

/// Describes how the final program result should be interpreted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ResultSpec {
    /// Read a single register as the result value.
    Scalar {
        /// Register that holds the scalar result.
        register: Register,
    },
    /// Read a contiguous array from memory using pointer and length registers.
    Array {
        /// Register containing the base pointer.
        ptr_register: Register,
        /// Register containing the element count.
        length_register: Register,
        /// Element width in bytes. Supported values are 1, 2, 4, and 8.
        elem_width: u8,
        /// Whether elements should be sign-extended when decoded.
        signed: bool,
    },
}

impl Default for ResultSpec {
    fn default() -> Self {
        Self::Scalar {
            register: Register::A0,
        }
    }
}

/// Decoded result emitted by the pipeline after execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FinalResult {
    /// Scalar result read directly from a register.
    Scalar {
        /// Register that was read.
        register: Register,
        /// Raw register contents before any further interpretation.
        raw_value: i64,
        /// Interpreted scalar value.
        value: i64,
    },
    /// Array result decoded from memory.
    Array {
        /// Register that supplied the base pointer.
        ptr_register: Register,
        /// Register that supplied the element count.
        length_register: Register,
        /// Raw pointer value before validation.
        raw_pointer: i64,
        /// Number of elements requested from memory.
        length: i64,
        /// Element width in bytes.
        elem_width: u8,
        /// Whether values were sign-extended while decoding.
        signed: bool,
        /// Decoded element values, empty when decoding failed.
        values: Vec<i64>,
        /// Decode error, if the requested array could not be read.
        error: Option<String>,
    },
}

/// Parses source text into an assembled program, or returns a parse/assembly error.
pub fn assemble_source(source: &str) -> Result<Assembler, String> {
    let pair = RizzParser::parse(Rule::program, source)
        .map_err(|err| format!("parse failed: {err}"))?
        .next()
        .ok_or_else(|| "parser returned no program".to_string())?;

    let mut assembler = Assembler::default();
    assembler
        .assemble(pair)
        .map_err(|err| format!("assembly failed: {err}"))?;
    assembler
        .resolve()
        .map_err(|err| format!("resolution failed: {err}"))?;
    Ok(assembler)
}

/// Executes an assembled program with the provided run configuration.
///
/// Returns the executor state after running up to `max_steps`; execution
/// errors are surfaced as strings.
pub fn execute_program(
    assembler: &Assembler,
    run_config: &RunConfig,
    max_steps: u64,
) -> Result<Executor, String> {
    let mut executor = Executor::default();
    executor
        .execute(assembler, run_config, Some(max_steps))
        .map_err(|err| format!("execution failed: {err}"))?;
    Ok(executor)
}

/// Runs parsing, assembly, analysis, execution, and result decoding in one step.
///
/// Fails if parsing, assembly, execution, or JSON serialization fails.
pub fn run_pipeline(
    source: &str,
    run_config: &RunConfig,
    result_spec: &ResultSpec,
    max_steps: u64,
) -> Result<RunArtifacts, String> {
    let assembler = assemble_source(source)?;
    let analysis = analyze(&assembler);
    let executor = execute_program(&assembler, run_config, max_steps)?;
    let result = interpret_result(&executor, result_spec);

    Ok(RunArtifacts {
        assembly: serde_json::to_value(assembler.assembled()).map_err(|err| err.to_string())?,
        trace: serde_json::to_value(executor.trace()).map_err(|err| err.to_string())?,
        analysis,
        result,
    })
}

/// Parses a `--reg` style assignment of the form `<register>=<value>`.
pub fn parse_register_assignment(raw: &str) -> Result<RegisterValue, String> {
    let (name, value) = raw
        .split_once('=')
        .ok_or_else(|| format!("invalid --reg value '{raw}', expected <register>=<value>"))?;
    parse_register_input(name.trim(), value.trim())
}

/// Parses a register name and integer value for initialization.
///
/// Rejects writes to the zero register and returns a descriptive error for
/// unknown register names or invalid integer values.
pub fn parse_register_input(name: &str, value: &str) -> Result<RegisterValue, String> {
    let register =
        Register::from_str(name).map_err(|_| format!("unknown register in input: {name}"))?;
    if register == Register::Zero {
        return Err("cannot initialize zero register".to_string());
    }
    let value = value
        .parse::<i64>()
        .map_err(|_| format!("invalid register value: {value}"))?;
    Ok(RegisterValue { register, value })
}

fn interpret_result(executor: &Executor, result_spec: &ResultSpec) -> FinalResult {
    let Some(state) = executor.final_state() else {
        return FinalResult::Scalar {
            register: Register::A0,
            raw_value: 0,
            value: 0,
        };
    };

    match result_spec {
        ResultSpec::Scalar { register } => {
            let value = state.reg_file.regs[register.index()];
            FinalResult::Scalar {
                register: *register,
                raw_value: value,
                value,
            }
        }
        ResultSpec::Array {
            ptr_register,
            length_register,
            elem_width,
            signed,
        } => {
            let raw_pointer = state.reg_file.regs[ptr_register.index()];
            let length = state.reg_file.regs[length_register.index()];

            match decode_array(executor.memory(), raw_pointer, length, *elem_width, *signed) {
                Ok(values) => FinalResult::Array {
                    ptr_register: *ptr_register,
                    length_register: *length_register,
                    raw_pointer,
                    length,
                    elem_width: *elem_width,
                    signed: *signed,
                    values,
                    error: None,
                },
                Err(error) => FinalResult::Array {
                    ptr_register: *ptr_register,
                    length_register: *length_register,
                    raw_pointer,
                    length,
                    elem_width: *elem_width,
                    signed: *signed,
                    values: Vec::new(),
                    error: Some(error),
                },
            }
        }
    }
}

fn decode_array(
    memory: &Memory,
    raw_pointer: i64,
    length: i64,
    elem_width: u8,
    signed: bool,
) -> Result<Vec<i64>, String> {
    if raw_pointer < 0 {
        return Err(format!("negative pointer value: {raw_pointer}"));
    }
    if length < 0 {
        return Err(format!("negative array length: {length}"));
    }
    if !matches!(elem_width, 1 | 2 | 4 | 8) {
        return Err(format!("unsupported element width: {elem_width}"));
    }

    let base = raw_pointer as u64;
    let mut values = Vec::with_capacity(length as usize);
    for index in 0..(length as u64) {
        let offset = index
            .checked_mul(u64::from(elem_width))
            .ok_or_else(|| "array offset overflow".to_string())?;
        let address = base
            .checked_add(offset)
            .ok_or_else(|| "array address overflow".to_string())?;
        let value = match (elem_width, signed) {
            (1, true) => memory.load8(address).map(|value| value as i8 as i64),
            (1, false) => memory.load8(address).map(|value| value as i64),
            (2, true) => memory.load16(address).map(|value| value as i16 as i64),
            (2, false) => memory.load16(address).map(|value| value as i64),
            (4, true) => memory.load32(address).map(|value| value as i32 as i64),
            (4, false) => memory.load32(address).map(|value| value as i64),
            (8, _) => memory.load64(address).map(|value| value as i64),
            _ => unreachable!(),
        }
        .map_err(|err| format!("failed to decode element {index} at {address:#x}: {err}"))?;
        values.push(value);
    }
    Ok(values)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::{Memory, MemoryEventKind};

    const LOOP_PROGRAM: &str = ".global main
main:
    li t0, 0
    li t1, 3
loop:
    addi t0, t0, 1
    blt t0, t1, loop
    ret
";

    const CALL_PROGRAM: &str = ".global main
main:
    jal ra, addone
    mv ra, zero
    ret

addone:
    li a0, 1
    ret
";

    const STACK_CALL_PROGRAM: &str = ".global main
main:
    jal ra, outer
    mv ra, zero
    ret

outer:
    addi sp, sp, -4
    sw ra, 0(sp)
    jal ra, inner
    lw ra, 0(sp)
    addi sp, sp, 4
    ret

inner:
    li a0, 7
    ret
";

    const SIGN_EXT_PROGRAM: &str = ".global main
main:
    addi sp, sp, -4
    li t0, -1
    sb t0, 0(sp)
    lb a0, 0(sp)
    lbu a1, 0(sp)
    addi sp, sp, 4
    ret
";

    const MAX3_PROGRAM: &str = ".global max3_s

# a0 - int a
# a1 - int b
# a2 - int c

max3_s:
    sd ra, (sp)
    jal max2
    ld ra, (sp)
    mv a1, a2
    sd ra, (sp)
    jal max2
    ld ra, (sp)
    j done


# a0 - int a
# a1 - int b
max2:
    bgt a0, a1, done
    mv a0, a1
    j done

done:
    ret
";

    const UNSIGNED_BRANCH_PROGRAM: &str = ".global main
main:
    li t0, -1
    li t1, 1
    bltu t1, t0, less
    li a0, 0
    ret
less:
    li a0, 1
    ret
";

    const ZERO_BRANCH_PROGRAM: &str = ".global main
main:
    li t0, 0
    beqz t0, zero_path
    li a0, -1
    ret
zero_path:
    li a0, 5
    ret
";

    const BLEU_PROGRAM: &str = ".global main
main:
    li t0, -1
    li t1, 1
    bleu t0, t1, wrong
    li a0, 7
    ret
wrong:
    li a0, 3
    ret
";

    #[test]
    fn resolves_backward_branch_labels() {
        let assembler = assemble_source(LOOP_PROGRAM).expect("assemble loop");
        let json = assembler.to_json().expect("json");
        assert!(json.contains("\"Address\": 8"));
    }

    #[test]
    fn executes_loop_until_branch_falls_through() {
        let assembler = assemble_source(LOOP_PROGRAM).expect("assemble loop");
        let executor =
            execute_program(&assembler, &RunConfig::default(), 100).expect("execute loop");
        let states = executor.trace().states.clone();
        let last = states.last().expect("final state");
        assert!(last.terminated);
        assert_eq!(last.reg_file.regs[5], 3);
        assert_eq!(last.pc, 16);
    }

    #[test]
    fn detects_cfg_loop_header() {
        let assembler = assemble_source(LOOP_PROGRAM).expect("assemble loop");
        let analysis = analyze(&assembler);
        assert_eq!(analysis.loops.len(), 1);
        assert_eq!(analysis.loops[0].blocks, vec![1]);
    }

    #[test]
    fn supports_jal_and_ret() {
        let assembler = assemble_source(CALL_PROGRAM).expect("assemble call");
        let executor =
            execute_program(&assembler, &RunConfig::default(), 100).expect("execute call");
        let last = executor.trace().states.last().expect("final state");
        assert!(last.terminated);
        assert_eq!(last.reg_file.regs[10], 1);
    }

    #[test]
    fn supports_stack_save_restore_around_nested_jal() {
        let assembler = assemble_source(STACK_CALL_PROGRAM).expect("assemble stack call");
        let executor =
            execute_program(&assembler, &RunConfig::default(), 200).expect("execute stack call");
        let states = &executor.trace().states;
        let last = states.last().expect("final state");
        assert!(last.terminated);
        assert_eq!(last.reg_file.regs[10], 7);
        assert_eq!(
            last.reg_file.regs[2],
            Memory::initial_stack_pointer() as i64
        );
        assert!(states.iter().any(|state| {
            state
                .memory_events
                .iter()
                .any(|event| event.kind == MemoryEventKind::Store && event.width == 4)
        }));
        assert!(states.iter().any(|state| {
            state
                .memory_events
                .iter()
                .any(|event| event.kind == MemoryEventKind::Load && event.width == 4)
        }));
    }

    #[test]
    fn records_signed_and_unsigned_load_values() {
        let assembler = assemble_source(SIGN_EXT_PROGRAM).expect("assemble sign extension");
        let executor = execute_program(&assembler, &RunConfig::default(), 100)
            .expect("execute sign extension");
        let last = executor.trace().states.last().expect("final state");
        assert!(last.terminated);
        assert_eq!(last.reg_file.regs[10], -1);
        assert_eq!(last.reg_file.regs[11], 255);
    }

    #[test]
    fn describes_every_supported_memory_instruction_exactly() {
        let program = ".global main
main:
    li t0, -1
    sb t0, 0(sp)
    sh t0, 2(sp)
    sw t0, 4(sp)
    sd t0, -8(sp)
    lb a0, 0(sp)
    lbu a1, 0(sp)
    lh a2, 2(sp)
    lhu a3, 2(sp)
    lw a4, 4(sp)
    ld a5, -8(sp)
    ret
";
        let assembler = assemble_source(program).expect("assemble memory operations");
        let executor = execute_program(&assembler, &RunConfig::default(), 100)
            .expect("execute memory operations");
        let events = executor
            .trace()
            .states
            .iter()
            .flat_map(|state| &state.memory_events)
            .collect::<Vec<_>>();

        assert_eq!(events.len(), 10);
        assert_eq!(
            events
                .iter()
                .map(|event| event.opcode.as_str())
                .collect::<Vec<_>>(),
            ["sb", "sh", "sw", "sd", "lb", "lbu", "lh", "lhu", "lw", "ld"]
        );

        let expected_raw = [
            "0xff",
            "0xffff",
            "0xffffffff",
            "0xffffffffffffffff",
            "0xff",
            "0xff",
            "0xffff",
            "0xffff",
            "0xffffffff",
            "0xffffffffffffffff",
        ];
        assert_eq!(
            events
                .iter()
                .map(|event| event.raw_value.as_str())
                .collect::<Vec<_>>(),
            expected_raw
        );
        assert_eq!(events[0].register, Register::T0);
        assert_eq!(events[0].register_value, "-1");
        assert_eq!(events[0].previous_raw_value.as_deref(), Some("0x00"));
        assert_eq!(
            events[3].previous_raw_value.as_deref(),
            Some("0x0000000000000000")
        );
        assert_eq!(events[4].register, Register::A0);
        assert_eq!(events[4].register_value, "-1");
        assert_eq!(events[5].register_value, "255");
        assert_eq!(events[7].register_value, "65535");
        assert_eq!(events[9].register_value, "-1");
        assert!(events
            .iter()
            .all(|event| event.address_hex.starts_with("0x")));
    }

    #[test]
    fn rejects_misaligned_word_access() {
        let program = ".global main
main:
    addi sp, sp, -8
    sw ra, 2(sp)
    ret
";
        let assembler = assemble_source(program).expect("assemble misaligned");
        let err = execute_program(&assembler, &RunConfig::default(), 20).expect_err("should fail");
        assert!(err.contains("misaligned"));
    }

    #[test]
    fn rejects_unknown_labels() {
        let bad = ".global main\nmain:\n    beq t0, t1, missing\n";
        let err = assemble_source(bad).expect_err("should fail");
        assert!(err.contains("unknown label"));
    }

    #[test]
    fn parses_register_assignment() {
        let reg = parse_register_assignment("a0=42").expect("parse reg");
        assert_eq!(reg.register, Register::A0);
        assert_eq!(reg.value, 42);
    }

    #[test]
    fn rejects_zero_register_assignment() {
        let err = parse_register_assignment("zero=5").expect_err("should reject");
        assert!(err.contains("zero register"));
    }

    #[test]
    fn injects_input_registers_into_step_zero_and_metadata() {
        let assembler = assemble_source(CALL_PROGRAM).expect("assemble call");
        let config = RunConfig {
            input_registers: vec![
                RegisterValue {
                    register: Register::A0,
                    value: 99,
                },
                RegisterValue {
                    register: Register::SP,
                    value: 4096,
                },
            ],
        };
        let executor = execute_program(&assembler, &config, 100).expect("execute call");
        let trace = executor.trace();
        assert_eq!(trace.run.input_registers, config.input_registers);
        assert_eq!(trace.states[0].reg_file.regs[10], 99);
        assert_eq!(trace.states[0].reg_file.regs[2], 4096);
    }

    #[test]
    fn parses_implicit_zero_memory_offset_and_pseudo_lowering() {
        let assembler = assemble_source(MAX3_PROGRAM).expect("assemble max3");
        let json = assembler.to_json().expect("json");
        assert!(json.contains("\"op_code\": \"SD\""));
        assert!(json.contains("\"offset\": 0"));
        assert!(json.contains("\"op_code\": \"BLT\""));
    }

    #[test]
    fn assembles_unsigned_and_zero_branch_forms() {
        let program = ".global main
main:
    bltu a0, a1, less
    bgeu a0, a1, more
    beqz t0, done
less:
more:
done:
    ret
";
        let assembler = assemble_source(program).expect("assemble branches");
        let json = assembler.to_json().expect("json");
        assert!(json.contains("\"op_code\": \"BLTU\""));
        assert!(json.contains("\"op_code\": \"BGEU\""));
        assert!(json.contains("\"op_code\": \"BEQ\""));
        assert!(json.contains("\"rs2\": \"Zero\""));
    }

    #[test]
    fn lowers_common_branch_pseudos() {
        let program = ".global main
main:
    bgt a0, a1, gt
    ble a0, a1, le
    bgtu a0, a1, gtu
    bleu a0, a1, leu
    bnez t0, nz
    bltz t1, neg
    bgez t2, ge
gt:
le:
gtu:
leu:
nz:
neg:
ge:
    ret
";
        let assembler = assemble_source(program).expect("assemble pseudo branches");
        let json = assembler.to_json().expect("json");
        assert!(json.contains("\"op_code\": \"BLT\""));
        assert!(json.contains("\"op_code\": \"BGE\""));
        assert!(json.contains("\"op_code\": \"BLTU\""));
        assert!(json.contains("\"op_code\": \"BGEU\""));
        assert!(json.contains("\"op_code\": \"BNE\""));
        assert!(json.contains("\"rs2\": \"Zero\""));
    }

    #[test]
    fn supports_doubleword_memory_round_trip() {
        let program = ".global main
main:
    sd a0, (sp)
    ld a1, (sp)
    ret
";
        let assembler = assemble_source(program).expect("assemble ld/sd");
        let config = RunConfig {
            input_registers: vec![RegisterValue {
                register: Register::A0,
                value: 0x1122_3344_5566_7788i64,
            }],
        };
        let executor = execute_program(&assembler, &config, 50).expect("execute ld/sd");
        let last = executor.trace().states.last().expect("final state");
        assert_eq!(last.reg_file.regs[11], 0x1122_3344_5566_7788i64);
        assert!(
            executor
                .trace()
                .states
                .iter()
                .any(|state| { state.memory_events.iter().any(|event| event.width == 8) })
        );
    }

    #[test]
    fn executes_unsigned_branch_with_u64_comparison() {
        let assembler = assemble_source(UNSIGNED_BRANCH_PROGRAM).expect("assemble unsigned");
        let executor =
            execute_program(&assembler, &RunConfig::default(), 50).expect("execute unsigned");
        let last = executor.trace().states.last().expect("final state");
        assert!(last.terminated);
        assert_eq!(last.reg_file.regs[10], 1);
    }

    #[test]
    fn executes_zero_branch_pseudo_via_lowering() {
        let assembler = assemble_source(ZERO_BRANCH_PROGRAM).expect("assemble zero branch");
        let executor =
            execute_program(&assembler, &RunConfig::default(), 50).expect("execute zero branch");
        let last = executor.trace().states.last().expect("final state");
        assert!(last.terminated);
        assert_eq!(last.reg_file.regs[10], 5);
    }

    #[test]
    fn executes_unsigned_pseudo_branch_with_correct_operand_order() {
        let assembler = assemble_source(BLEU_PROGRAM).expect("assemble bleu");
        let executor =
            execute_program(&assembler, &RunConfig::default(), 50).expect("execute bleu");
        let last = executor.trace().states.last().expect("final state");
        assert!(last.terminated);
        assert_eq!(last.reg_file.regs[10], 7);
    }

    #[test]
    fn runs_max3_program_as_written() {
        let assembler = assemble_source(MAX3_PROGRAM).expect("assemble max3");
        let config = RunConfig {
            input_registers: vec![
                RegisterValue {
                    register: Register::A0,
                    value: 3,
                },
                RegisterValue {
                    register: Register::A1,
                    value: 9,
                },
                RegisterValue {
                    register: Register::A2,
                    value: 7,
                },
            ],
        };
        let executor = execute_program(&assembler, &config, 200).expect("execute max3");
        let last = executor.trace().states.last().expect("final state");
        assert!(last.terminated);
        assert_eq!(last.reg_file.regs[10], 9);
    }

    #[test]
    fn defaults_final_result_to_scalar_a0() {
        let artifacts = run_pipeline(
            CALL_PROGRAM,
            &RunConfig::default(),
            &ResultSpec::default(),
            100,
        )
        .expect("run pipeline");
        assert_eq!(
            artifacts.result,
            FinalResult::Scalar {
                register: Register::A0,
                raw_value: 1,
                value: 1,
            }
        );
    }

    #[test]
    fn interprets_array_result_from_a0_and_a1() {
        let program = ".global main
main:
    addi sp, sp, -16
    li t0, 7
    li t1, 9
    sw t0, 0(sp)
    sw t1, 4(sp)
    mv a0, sp
    li a1, 2
    ret
";
        let artifacts = run_pipeline(
            program,
            &RunConfig::default(),
            &ResultSpec::Array {
                ptr_register: Register::A0,
                length_register: Register::A1,
                elem_width: 4,
                signed: true,
            },
            100,
        )
        .expect("run array pipeline");

        assert_eq!(
            artifacts.result,
            FinalResult::Array {
                ptr_register: Register::A0,
                length_register: Register::A1,
                raw_pointer: (Memory::initial_stack_pointer() - 16) as i64,
                length: 2,
                elem_width: 4,
                signed: true,
                values: vec![7, 9],
                error: None,
            }
        );
    }

    #[test]
    fn reports_array_result_decode_errors_without_failing_run() {
        let program = ".global main
main:
    li a0, 4096
    li a1, -2
    ret
";
        let artifacts = run_pipeline(
            program,
            &RunConfig::default(),
            &ResultSpec::Array {
                ptr_register: Register::A0,
                length_register: Register::A1,
                elem_width: 4,
                signed: true,
            },
            50,
        )
        .expect("run error pipeline");

        match artifacts.result {
            FinalResult::Array { error, values, .. } => {
                assert!(error.expect("error").contains("negative array length"));
                assert!(values.is_empty());
            }
            _ => panic!("expected array result"),
        }
    }
}
