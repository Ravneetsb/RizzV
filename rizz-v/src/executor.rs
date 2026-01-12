use std::collections::{HashMap, HashSet};

use crate::Instruction;
use crate::assembler::Assembler;
use crate::cpu::CPU;
use crate::reg::*;
use crate::state::States;

use serde::Serialize;

#[derive(Debug, Default, PartialEq, Eq, Serialize)]
pub struct Executor {
    cpu: CPU,
    program: States,
}

impl Executor {
    pub fn execute(&mut self, assembler: &mut Assembler) {
        let mut instructions = assembler.iter();
        while let Some(inst) = instructions.next() {
            println!("{:?}", inst);
        }
    }
}
