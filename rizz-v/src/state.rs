use crate::instruction::Instruction;
use crate::reg::RegFile;
use serde::Serialize;

#[derive(Debug, Serialize, Eq, PartialEq)]
pub struct State {
    pub step: u8,
    pub reg_file: RegFile,
    pub instr: Instruction,
}

impl Default for State {
    fn default() -> Self {
        Self {
            step: 1,
            reg_file: RegFile::default(),
            instr: Instruction::Nil,
        }
    }
}

#[derive(Debug, Serialize, Eq, PartialEq)]
pub struct States {
    pub states: Vec<State>,
    pub count: u8,
}

impl Default for States {
    fn default() -> Self {
        Self {
            states: vec![State::default()],
            count: 1,
        }
    }
}

impl States {
    pub fn add(&mut self, state: State) {
        self.states.push(state);
        self.count += 1;
    }
}
