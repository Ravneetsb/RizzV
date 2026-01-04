use crate::instruction::Instruction;
use crate::reg::RegFile;
use serde::Serialize;

#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct State {
    pub step: u8,
    pub reg_file: RegFile,
    // instr: &'a str,
    pub instr: Instruction,
}

#[derive(Debug, Serialize, Default)]
pub struct States {
    pub states: Vec<State>,
    pub count: u8,
}

impl States {
    pub fn add(&mut self, state: State) {
        self.states.push(state);
        self.count += 1;
    }
}
