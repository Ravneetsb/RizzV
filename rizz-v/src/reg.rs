use serde::Serialize;
use strum_macros::{Display, EnumIter, EnumString};

#[derive(Copy, Debug, Eq, PartialEq, Hash, EnumIter, EnumString, Display, Serialize, Clone)]
#[strum(serialize_all = "lowercase")]
#[repr(u8)]
pub enum Register {
    Zero = 0,
    RA = 1,
    SP = 2,
    T0 = 5,
    T1 = 6,
    T2 = 7,
    S0 = 8,
    S1 = 9,
    A0 = 10,
    A1 = 11,
    A2 = 12,
    A3 = 13,
    A4 = 14,
    A5 = 15,
    A6 = 16,
    A7 = 17,
    S2 = 18,
    S3 = 19,
    S4 = 20,
    S5 = 21,
    S6 = 22,
    S7 = 23,
    S8 = 24,
    S9 = 25,
    S10 = 26,
    S11 = 27,
    T3 = 28,
    T4 = 29,
    T5 = 30,
    T6 = 31,
}

impl Register {
    #[inline]
    pub fn index(self) -> usize {
        self as usize
    }
}

#[derive(Debug, Default, PartialEq, Eq, Serialize, Clone)]
pub struct RegFile {
    pub regs: [i64; 32],
}

pub trait File {
    fn get(&self, r: Register) -> i64;
    fn set(&mut self, r: Register, v: i64);
}

impl File for RegFile {
    fn get(&self, r: Register) -> i64 {
        self.regs[r.index()]
    }

    fn set(&mut self, r: Register, v: i64) {
        let index = r.index();
        if index == 0 {
            return;
        }
        self.regs[index] = v;
    }
}

#[derive(Copy, Debug, Eq, PartialEq, Hash, EnumIter, EnumString, Display, Serialize, Clone)]
#[strum(serialize_all = "lowercase")]
pub enum ROpCode {
    ADD,
    SUB,
    DIV,
    MUL,
    SLL,
}

#[derive(Copy, Debug, Eq, PartialEq, Hash, EnumIter, EnumString, Display, Serialize, Clone)]
#[strum(serialize_all = "lowercase")]
pub enum IOpCode {
    ADDI,
}

#[derive(Copy, Debug, Eq, PartialEq, Hash, EnumIter, EnumString, Display, Serialize, Clone)]
#[strum(serialize_all = "lowercase")]
pub enum LoadOpCode {
    LB,
    LBU,
    LH,
    LHU,
    LW,
    LD,
}

#[derive(Copy, Debug, Eq, PartialEq, Hash, EnumIter, EnumString, Display, Serialize, Clone)]
#[strum(serialize_all = "lowercase")]
pub enum StoreOpCode {
    SB,
    SH,
    SW,
    SD,
}

#[derive(Copy, Debug, Eq, PartialEq, Hash, EnumIter, EnumString, Display, Serialize, Clone)]
#[strum(serialize_all = "lowercase")]
pub enum BOpCode {
    BEQ,
    BNE,
    BLT,
    BGE,
    BLTU,
    BGEU,
}

#[derive(Copy, Debug, Eq, PartialEq, Hash, EnumIter, EnumString, Display, Serialize, Clone)]
#[strum(serialize_all = "lowercase")]
pub enum JOpCode {
    JAL,
    JALR,
}

#[derive(Debug, Eq, PartialEq, Hash, EnumIter, EnumString, Display, Serialize, Clone)]
#[strum(serialize_all = "lowercase")]
pub enum PseudoCode {
    MV,
    LI,
    RET,
}
