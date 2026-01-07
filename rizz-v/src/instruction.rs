use crate::reg::*;
use serde::Serialize;

#[derive(Debug, Eq, PartialEq, Clone, Serialize)]
pub enum Instruction {
    R(RInst),
    I(IInst),
    Nil,
}

impl Default for Instruction {
    fn default() -> Self {
        Self::Nil
    }
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
