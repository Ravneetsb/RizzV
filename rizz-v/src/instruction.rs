use crate::reg::*;
use serde::Serialize;

#[derive(Debug, Eq, PartialEq, Clone, Serialize)]
pub enum Instruction {
    R(RInst),
    I(IInst),
    Nil,
    J(JInst),
}

impl Default for Instruction {
    fn default() -> Self {
        Self::Nil
    }
}

impl Instruction {
    pub fn r_inst(op_code: ROpCode, rd: Register, rs1: Register, rs2: Register) -> Self {
        Instruction::R(RInst {
            op_code,
            rd,
            rs1,
            rs2,
        })
    }

    pub fn i_inst(op_code: IOpCode, rd: Register, rs: Register, imm: i32) -> Self {
        Instruction::I(IInst {
            op_code,
            rd,
            rs,
            imm,
        })
    }

    pub fn nil() -> Self {
        Instruction::Nil
    }

    pub fn ret() -> Self {
        Instruction::J(JInst {
            op_code: JOpCode::JALR,
            rd: Register::Zero,
            address: Memory {
                register: Register::RA,
                offset: 0,
            },
        })
    }
}

#[derive(Debug, Eq, PartialEq, Serialize, Clone)]
pub struct Memory {
    pub register: Register,
    pub offset: i32,
}

#[derive(Debug, Eq, PartialEq, Serialize, Clone)]
pub struct JInst {
    pub op_code: JOpCode,
    pub rd: Register,
    pub address: Memory,
}
#[derive(Debug, Eq, PartialEq, Serialize, Clone)]
pub struct RInst {
    pub op_code: ROpCode,
    pub rd: Register,
    pub rs1: Register,
    pub rs2: Register,
}

#[derive(Debug, Eq, PartialEq, Serialize, Clone)]
pub struct IInst {
    pub op_code: IOpCode,
    pub rd: Register,
    pub rs: Register,
    pub imm: i32,
}
