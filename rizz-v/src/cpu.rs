use crate::reg::{File, RegFile, Register};
use serde::Serialize;

#[derive(Debug, Default, PartialEq, Eq, Serialize, Clone)]
pub struct CPU {
    pc: u64,
    pub regs: RegFile,
}

impl CPU {
    pub fn increment_byte(&mut self) {
        self.pc += 4;
    }

    pub fn set_pc(&mut self, value: u64) {
        self.pc = value;
    }
    pub fn get_pc(&mut self) -> u64 {
        self.pc
    }

    pub fn next_byte(&mut self) -> u64 {
        self.pc + 4
    }
}

impl CPU {
    pub fn get_reg_value(&mut self, r: Register) -> i64 {
        self.regs.get(r)
    }

    pub fn set_reg_value(&mut self, r: Register, v: i64) {
        self.regs.set(r, v);
    }
}
