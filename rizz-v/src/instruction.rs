use crate::reg::*;
use serde::Serialize;

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct Instruction {
    pub op_code: OpCode,
    pub rd: Register,
    pub rs1: Register,
    pub rs2: Register,
}
