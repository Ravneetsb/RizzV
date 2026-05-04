use crate::instruction::Instruction;
use crate::memory::MemoryEvent;
use crate::reg::{RegFile, Register};
use serde::Serialize;

#[derive(Debug, Serialize, Eq, PartialEq, Clone)]
pub struct State {
    pub step: u64,
    pub pc: u64,
    pub next_pc: u64,
    pub reg_file: RegFile,
    pub instr: Instruction,
    pub branch_taken: Option<bool>,
    pub memory_events: Vec<MemoryEvent>,
    pub terminated: bool,
}

impl State {
    pub fn initial(pc: u64, reg_file: RegFile) -> Self {
        Self {
            step: 0,
            pc,
            next_pc: pc,
            reg_file,
            instr: Instruction::Nil,
            branch_taken: None,
            memory_events: Vec::new(),
            terminated: false,
        }
    }
}

#[derive(Debug, Serialize, Eq, PartialEq, Clone)]
pub struct RunMetadata {
    pub entry_pc: u64,
    pub entry_label: Option<String>,
    pub input_registers: Vec<RegisterValue>,
}

#[derive(Debug, Serialize, Eq, PartialEq, Clone)]
pub struct RegisterValue {
    pub register: Register,
    pub value: i64,
}

#[derive(Debug, Serialize, Eq, PartialEq, Clone)]
pub struct Trace {
    pub run: RunMetadata,
    pub states: Vec<State>,
    pub count: u64,
}

impl Default for Trace {
    fn default() -> Self {
        Self {
            run: RunMetadata {
                entry_pc: 0,
                entry_label: None,
                input_registers: Vec::new(),
            },
            states: Vec::new(),
            count: 0,
        }
    }
}

impl Trace {
    pub fn add(&mut self, state: State) {
        self.count += 1;
        self.states.push(state);
    }
}
