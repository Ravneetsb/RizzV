use derive_more::Display as MoreDisplay;
use std::collections::{HashMap, HashSet};
use std::error::Error;

use crate::Instruction;
use crate::assembler::Assembler;
use crate::cpu::CPU;
use crate::instruction::{IInst, JInst, RInst};
use crate::reg::*;
use crate::state::{State, States};

use serde::Serialize;

#[derive(Debug, Default, PartialEq, Eq, Serialize)]
pub struct Executor {
    cpu: CPU,
    program: States,
}

#[derive(Debug, MoreDisplay)]
pub enum ExecErr {
    #[display("Invalid binary operands: {lhs} . {rhs}")]
    InvalidBinOperands { lhs: String, rhs: String },
    #[display("Invalid Memory Location: {msg}")]
    InvalidMemory { msg: String },
}

impl Executor {
    pub fn execute(&mut self, assembler: &mut Assembler) -> Result<(), ExecErr> {
        let mut instructions = assembler.iter();
        while let Some(inst) = instructions.next() {
            match inst {
                Instruction::R(rinst) => self.execute_r(rinst.to_owned())?,
                Instruction::I(iinst) => self.execute_i(iinst.to_owned())?,
                Instruction::Nil => unreachable!(),
                Instruction::J(jinst) => self.execute_j(jinst.to_owned())?,
            }
        }
        Ok(())
    }

    fn execute_r(&mut self, inst: RInst) -> Result<(), ExecErr> {
        let op_code = inst.op_code;
        let rd = inst.rd;
        let rs1 = inst.rs1;
        let rs2 = inst.rs2;
        match op_code {
            ROpCode::ADD => {
                let sum = self.cpu.get_reg_value(rs1.clone()) + self.cpu.get_reg_value(rs2.clone());
                self.cpu.set_reg_value(rd.clone(), sum);
            }
            ROpCode::SUB => {
                let res = self.cpu.get_reg_value(rs1.clone()) - self.cpu.get_reg_value(rs2.clone());
                self.cpu.set_reg_value(rd.clone(), res);
            }
            ROpCode::DIV => {
                let res = self.cpu.get_reg_value(rs1.clone()) / self.cpu.get_reg_value(rs2.clone());
                self.cpu.set_reg_value(rd.clone(), res);
            }
            ROpCode::MUL => {
                let res = self.cpu.get_reg_value(rs1.clone()) * self.cpu.get_reg_value(rs2.clone());
                self.cpu.set_reg_value(rd.clone(), res);
            }
            ROpCode::SLL => {
                let res =
                    self.cpu.get_reg_value(rs1.clone()) << self.cpu.get_reg_value(rs2.clone());
                self.cpu.set_reg_value(rd.clone(), res);
            }
        }
        let state = State {
            step: self.program.count + 1,
            reg_file: self.cpu.regs.clone(),
            instr: Instruction::R(inst),
        };
        self.program.add(state);
        Ok(())
    }
    fn execute_i(&mut self, inst: IInst) -> Result<(), ExecErr> {
        let op_code = inst.op_code;
        let rd = inst.rd;
        let rs = inst.rs;
        let imm = inst.imm;
        match op_code {
            IOpCode::ADDI => {
                let sum = self.cpu.get_reg_value(rs) + imm as i64;
                self.cpu.set_reg_value(rd, sum);
            }
        }
        let state = State {
            step: self.program.count + 1,
            reg_file: self.cpu.regs.clone(),
            instr: Instruction::I(inst),
        };
        self.program.add(state);
        Ok(())
    }
    fn execute_j(&mut self, inst: JInst) -> Result<(), ExecErr> {
        todo!();
    }
}

impl Executor {
    pub fn to_json(&mut self) {
        let json = serde_json::to_string_pretty(&self.program).unwrap();
        println!("{}", json);
    }
}
