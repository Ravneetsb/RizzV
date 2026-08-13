use derive_more::Display as MoreDisplay;
use serde::Serialize;

use crate::assembler::Assembler;
use crate::cpu::CPU;
use crate::instruction::{
    BInst, IInst, Instruction, JInst, JumpTarget, LInst, MemoryRef, RInst, SInst,
};
use crate::memory::{Memory, MemoryError, MemoryEvent, MemoryEventKind};
use crate::reg::{BOpCode, IOpCode, JOpCode, LoadOpCode, ROpCode, Register, StoreOpCode};
use crate::state::{RegisterValue, RunMetadata, State, Trace};

const WORD_SIZE: u64 = 4;
const DEFAULT_MAX_STEPS: u64 = 10_000;

#[derive(Debug, Default, PartialEq, Eq, Serialize, Clone)]
pub struct RunConfig {
    pub input_registers: Vec<RegisterValue>,
}

#[derive(Debug, Default, PartialEq, Eq, Serialize)]
pub struct Executor {
    cpu: CPU,
    #[serde(skip_serializing)]
    memory: Memory,
    trace: Trace,
}

#[derive(Debug, MoreDisplay)]
pub enum ExecErr {
    #[display("instruction at invalid pc: {pc}")]
    InvalidPc { pc: u64 },
    #[display("unresolved target at pc: {pc}")]
    UnresolvedTarget { pc: u64 },
    #[display("max step count exceeded: {limit}")]
    MaxStepsExceeded { limit: u64 },
    #[display("memory error: {err}")]
    Memory { err: MemoryError },
}

impl From<MemoryError> for ExecErr {
    fn from(err: MemoryError) -> Self {
        Self::Memory { err }
    }
}

impl Executor {
    pub fn execute(
        &mut self,
        assembler: &Assembler,
        config: &RunConfig,
        max_steps: Option<u64>,
    ) -> Result<(), ExecErr> {
        let limit = max_steps.unwrap_or(DEFAULT_MAX_STEPS);
        let entry_pc = assembler.entry_pc();
        self.cpu = CPU::default();
        self.memory = Memory::default();
        self.cpu.set_pc(entry_pc);
        self.cpu
            .set_reg_value(Register::SP, Memory::initial_stack_pointer() as i64);
        for input in &config.input_registers {
            self.cpu.set_reg_value(input.register, input.value);
        }

        self.trace = Trace {
            run: RunMetadata {
                entry_pc,
                entry_label: assembler.entry_label(),
                input_registers: config.input_registers.clone(),
            },
            ..Trace::default()
        };
        self.trace
            .add(State::initial(entry_pc, self.cpu.regs.clone()));

        let mut steps = 0;
        loop {
            if steps >= limit {
                return Err(ExecErr::MaxStepsExceeded { limit });
            }

            let pc = self.cpu.get_pc();
            let Some(instruction) = assembler.instruction_at_pc(pc).cloned() else {
                return Err(ExecErr::InvalidPc { pc });
            };

            let (next_pc, branch_taken, memory_events, terminated) = match &instruction {
                Instruction::R(inst) => (self.execute_r(inst), None, Vec::new(), false),
                Instruction::I(inst) => (self.execute_i(inst), None, Vec::new(), false),
                Instruction::L(inst) => {
                    let (next_pc, event) = self.execute_l(inst)?;
                    (next_pc, None, vec![event], false)
                }
                Instruction::S(inst) => {
                    let (next_pc, event) = self.execute_s(inst)?;
                    (next_pc, None, vec![event], false)
                }
                Instruction::B(inst) => {
                    let (next_pc, branch_taken, terminated) = self.execute_b(inst, pc)?;
                    (next_pc, branch_taken, Vec::new(), terminated)
                }
                Instruction::J(inst) => {
                    let (next_pc, branch_taken, terminated) = self.execute_j(inst, pc)?;
                    (next_pc, branch_taken, Vec::new(), terminated)
                }
                Instruction::Nil => (pc + WORD_SIZE, None, Vec::new(), false),
            };

            self.cpu.set_pc(next_pc);
            steps += 1;
            self.trace.add(State {
                step: steps,
                pc,
                next_pc,
                reg_file: self.cpu.regs.clone(),
                instr: instruction,
                branch_taken,
                memory_events,
                terminated,
            });

            if terminated {
                break;
            }
        }
        Ok(())
    }

    fn execute_r(&mut self, inst: &RInst) -> u64 {
        let lhs = self.cpu.get_reg_value(inst.rs1);
        let rhs = self.cpu.get_reg_value(inst.rs2);
        let value = match inst.op_code {
            ROpCode::ADD => lhs + rhs,
            ROpCode::SUB => lhs - rhs,
            ROpCode::DIV => lhs / rhs,
            ROpCode::MUL => lhs * rhs,
            ROpCode::SLL => lhs << rhs,
        };
        self.cpu.set_reg_value(inst.rd, value);
        self.cpu.get_pc() + WORD_SIZE
    }

    fn execute_i(&mut self, inst: &IInst) -> u64 {
        let value = match inst.op_code {
            IOpCode::ADDI => self.cpu.get_reg_value(inst.rs) + i64::from(inst.imm),
        };
        self.cpu.set_reg_value(inst.rd, value);
        self.cpu.get_pc() + WORD_SIZE
    }

    fn execute_l(&mut self, inst: &LInst) -> Result<(u64, MemoryEvent), ExecErr> {
        let address = self.resolve_address(&inst.address);
        let (value, width, raw_value) = match inst.op_code {
            LoadOpCode::LB => {
                let raw = self.memory.load8(address)?;
                (raw as i8 as i64, 1, u64::from(raw))
            }
            LoadOpCode::LBU => {
                let raw = self.memory.load8(address)?;
                (i64::from(raw), 1, u64::from(raw))
            }
            LoadOpCode::LH => {
                let raw = self.memory.load16(address)?;
                (raw as i16 as i64, 2, u64::from(raw))
            }
            LoadOpCode::LHU => {
                let raw = self.memory.load16(address)?;
                (i64::from(raw), 2, u64::from(raw))
            }
            LoadOpCode::LW => {
                let raw = self.memory.load32(address)?;
                (raw as i32 as i64, 4, u64::from(raw))
            }
            LoadOpCode::LD => {
                let raw = self.memory.load64(address)?;
                (raw as i64, 8, raw)
            }
        };
        self.cpu.set_reg_value(inst.rd, value);
        Ok((
            self.cpu.get_pc() + WORD_SIZE,
            MemoryEvent {
                kind: MemoryEventKind::Load,
                opcode: inst.op_code.to_string(),
                register: inst.rd,
                address,
                address_hex: format!("{address:#x}"),
                width,
                value,
                register_value: value.to_string(),
                raw_value: format_raw_value(raw_value, width),
                previous_value: None,
                previous_raw_value: None,
            },
        ))
    }

    fn execute_s(&mut self, inst: &SInst) -> Result<(u64, MemoryEvent), ExecErr> {
        let address = self.resolve_address(&inst.address);
        let source = self.cpu.get_reg_value(inst.rs);
        let (width, value, previous_value) = match inst.op_code {
            StoreOpCode::SB => {
                let value = source as u8;
                let previous = self.memory.store8(address, value)?;
                (1, value as i8 as i64, Some(previous))
            }
            StoreOpCode::SH => {
                let value = source as u16;
                let previous = self.memory.store16(address, value)?;
                (2, value as i16 as i64, Some(previous))
            }
            StoreOpCode::SW => {
                let value = source as u32;
                let previous = self.memory.store32(address, value)?;
                (4, value as i32 as i64, Some(previous))
            }
            StoreOpCode::SD => {
                let value = source as u64;
                let previous = self.memory.store64(address, value)?;
                (8, value as i64, Some(previous))
            }
        };
        Ok((
            self.cpu.get_pc() + WORD_SIZE,
            MemoryEvent {
                kind: MemoryEventKind::Store,
                opcode: inst.op_code.to_string(),
                register: inst.rs,
                address,
                address_hex: format!("{address:#x}"),
                width,
                value,
                register_value: source.to_string(),
                raw_value: format_raw_value(value as u64, width),
                previous_value,
                previous_raw_value: previous_value
                    .map(|previous| format_raw_value(previous as u64, width)),
            },
        ))
    }

    fn execute_b(&mut self, inst: &BInst, pc: u64) -> Result<(u64, Option<bool>, bool), ExecErr> {
        let lhs = self.cpu.get_reg_value(inst.rs1);
        let rhs = self.cpu.get_reg_value(inst.rs2);
        let taken = match inst.op_code {
            BOpCode::BEQ => lhs == rhs,
            BOpCode::BNE => lhs != rhs,
            BOpCode::BLT => lhs < rhs,
            BOpCode::BGE => lhs >= rhs,
            BOpCode::BLTU => (lhs as u64) < (rhs as u64),
            BOpCode::BGEU => (lhs as u64) >= (rhs as u64),
        };
        let target = inst
            .target
            .address()
            .ok_or(ExecErr::UnresolvedTarget { pc })?;
        let next_pc = if taken { target } else { pc + WORD_SIZE };
        Ok((next_pc, Some(taken), false))
    }

    fn execute_j(&mut self, inst: &JInst, pc: u64) -> Result<(u64, Option<bool>, bool), ExecErr> {
        let return_pc = pc + WORD_SIZE;
        match (&inst.op_code, &inst.target) {
            (JOpCode::JAL, JumpTarget::Direct(target)) => {
                let target = target.address().ok_or(ExecErr::UnresolvedTarget { pc })?;
                self.cpu.set_reg_value(inst.rd, return_pc as i64);
                Ok((target, Some(true), false))
            }
            (JOpCode::JALR, JumpTarget::Indirect(target)) => {
                let destination = self.resolve_address(target);
                self.cpu.set_reg_value(inst.rd, return_pc as i64);
                let terminated = inst.rd == Register::Zero && destination == 0;
                Ok((destination, Some(true), terminated))
            }
            _ => Err(ExecErr::UnresolvedTarget { pc }),
        }
    }

    fn resolve_address(&self, address: &MemoryRef) -> u64 {
        (self.cpu.get_reg_value(address.register) + i64::from(address.offset)) as u64
    }

    pub fn trace(&self) -> &Trace {
        &self.trace
    }

    pub fn memory(&self) -> &Memory {
        &self.memory
    }

    pub fn final_state(&self) -> Option<&State> {
        self.trace.states.last()
    }

    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(&self.trace)
    }
}

fn format_raw_value(value: u64, width: u8) -> String {
    let digits = usize::from(width) * 2;
    let masked = match width {
        1 => value & u64::from(u8::MAX),
        2 => value & u64::from(u16::MAX),
        4 => value & u64::from(u32::MAX),
        8 => value,
        _ => value,
    };
    format!("0x{masked:0digits$x}")
}
