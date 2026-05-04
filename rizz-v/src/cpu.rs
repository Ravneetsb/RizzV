use crate::reg::{File, RegFile, Register};
use serde::Serialize;

#[derive(Debug, Default, PartialEq, Eq, Serialize, Clone)]
pub struct CPU {
    pc: u64,
    pub regs: RegFile,
}

impl CPU {
    pub fn set_pc(&mut self, value: u64) {
        self.pc = value;
    }

    pub fn get_pc(&self) -> u64 {
        self.pc
    }

    pub fn next_pc(&self) -> u64 {
        self.pc + 4
    }

    pub fn get_reg_value(&self, register: Register) -> i64 {
        self.regs.get(register)
    }

    pub fn set_reg_value(&mut self, register: Register, value: i64) {
        self.regs.set(register, value);
    }
}
