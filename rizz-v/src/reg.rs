use core::error;
use serde::Serialize;
use std::collections::HashMap as Map;
use strum::IntoEnumIterator;
use strum_macros::{Display, EnumIter, EnumString};

#[derive(Debug, Eq, PartialEq, Hash, EnumIter, EnumString, Display, Serialize, Clone)]
#[strum(serialize_all = "lowercase")]
#[repr(u8)]
pub enum Register {
    Zero = 0,
    T0 = 5,
    T1 = 6,
    T2 = 7,
    A0 = 10,
    A1 = 11,
    A2 = 12,
    A3 = 13,
    A4 = 14,
    A5 = 15,
    A6 = 16,
    A7 = 17,
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
        let i = r.index();
        if i == 0 {
            panic!("Cannot assign value to Zero register");
        }
        self.regs[i] = v;
    }
}

#[derive(Debug, Eq, PartialEq, Hash, EnumIter, EnumString, Display, Serialize, Clone)]
#[strum(serialize_all = "lowercase")]
pub enum OpCode {
    ADD,
    SUB,
    DIV,
    MUL,
    SLL,
}
