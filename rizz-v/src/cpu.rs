use crate::reg::{File, RegFile, Register};
use serde::Serialize;

/// Architectural CPU state tracked by the executor.
#[derive(Debug, Default, PartialEq, Eq, Serialize, Clone)]
pub struct CPU {
    /// Program counter in bytes.
    pc: u64,
    /// General-purpose register file.
    pub regs: RegFile,
}

impl CPU {
    /// Sets the program counter to an absolute byte address.
    pub fn set_pc(&mut self, value: u64) {
        self.pc = value;
    }

    pub fn get_pc(&self) -> u64 {
        self.pc
    }

    pub fn next_pc(&self) -> u64 {
        self.pc + 4
    }

    /// Reads the current value of a register.
    pub fn get_reg_value(&self, register: Register) -> i64 {
        self.regs.get(register)
    }

    /// Writes a register value through the CPU register file.
    pub fn set_reg_value(&mut self, register: Register, value: i64) {
        self.regs.set(register, value);
    }
}
