use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::num::ParseIntError;
use std::str::FromStr;

use derive_more::Display as MoreDisplay;
use pest::iterators::Pair;
use serde::Serialize;
use strum_macros::{Display, EnumIter, EnumString};

use crate::Rule;
use crate::cpu::CPU;
use crate::instruction::Instruction;
use crate::reg::{IOpCode, PseudoCode, ROpCode, Register};
use serde_json;

#[derive(Debug, MoreDisplay)]
pub enum AsmError {
    #[display("missing {name}")]
    Missing { name: &'static str },
    #[display("invalid register count for {kind} instruction: expected {expected}, found {found}")]
    InvalidRegisterCount {
        kind: &'static str,
        expected: usize,
        found: usize,
    },
    #[display("unknown label: {msg}")]
    UnknownLabel { msg: String },
    #[display("parse error: {msg}")]
    Parse { msg: String },
    #[display("json error: {err}")]
    Json { err: serde_json::Error },
}

impl Error for AsmError {}

impl From<strum::ParseError> for AsmError {
    fn from(err: strum::ParseError) -> Self {
        Self::Parse {
            msg: err.to_string(),
        }
    }
}

impl From<ParseIntError> for AsmError {
    fn from(err: ParseIntError) -> Self {
        Self::Parse {
            msg: err.to_string(),
        }
    }
}

impl From<serde_json::Error> for AsmError {
    fn from(err: serde_json::Error) -> Self {
        Self::Json { err }
    }
}

#[derive(Debug, Default, PartialEq, Eq, Clone, Serialize)]
pub struct Assembler {
    program: Vec<Instruction>,
    pc: u64,
    labels: HashMap<String, u64>,
    globals: HashSet<String>,
}

#[derive(Debug, Eq, PartialEq, Hash, EnumIter, EnumString, Display, Serialize, Clone)]
#[strum(serialize_all = "lowercase")]
pub enum Directive {
    GLOBAL,
}

fn require_registes<const N: usize>(
    regs: Vec<Register>,
    kind: &'static str,
) -> Result<[Register; N], AsmError> {
    let found = regs.len();
    regs.try_into().map_err(|_| AsmError::InvalidRegisterCount {
        kind,
        expected: N,
        found,
    })
}

impl Assembler {
    pub fn assemble(&mut self, pair: Pair<Rule>) -> Result<(), AsmError> {
        match pair.as_rule() {
            Rule::instruction => {
                let inst = Assembler::create_inst(pair)?;
                self.program.push(inst);
                self.pc += 4;
                Ok(())
            }
            Rule::directive => {
                let mut inner = pair.into_inner();
                let directive = Directive::from_str(
                    inner
                        .next()
                        .ok_or(AsmError::Missing { name: "directive" })?
                        .as_str(),
                )?;
                let label = inner
                    .next()
                    .ok_or(AsmError::Missing { name: "Label" })?
                    .as_str();
                match directive {
                    Directive::GLOBAL => {
                        self.globals.insert(label.to_string());
                        Ok(())
                    }
                }
            }
            Rule::label => {
                let mut inner = pair.into_inner();
                let lbl = inner
                    .next()
                    .ok_or(AsmError::Missing { name: "Label" })?
                    .as_str()
                    .trim()
                    .to_string();
                self.labels.insert(lbl, self.pc);
                Ok(())
            }
            _ => {
                for inner in pair.into_inner() {
                    self.assemble(inner)?;
                }
                Ok(())
            }
        }
    }

    fn create_inst(pair: Pair<Rule>) -> Result<Instruction, AsmError> {
        let mut inner = pair.into_inner();
        let op_token = inner
            .next()
            .ok_or(AsmError::Missing { name: "Op token" })?
            .as_str()
            .trim()
            .to_string();
        let ops = inner
            .next()
            .and_then(|operands| operands.into_inner().next());

        if let Some(operands) = ops {
            let inst = match operands.as_rule() {
                Rule::r_operand => {
                    let op_code = ROpCode::from_str(&op_token)?;
                    let mut registers = operands.into_inner();
                    let regs: Vec<Register> = registers
                        .by_ref()
                        .take(3)
                        .map(|p| Register::from_str(p.as_str().trim()))
                        .collect::<Result<Vec<_>, _>>()?;

                    let [rd, rs1, rs2] = require_registes(regs, "R-type")?;
                    Instruction::r_inst(op_code, rd, rs1, rs2)
                }
                Rule::i_operand => {
                    let op_code = IOpCode::from_str(&op_token)?;
                    let mut registers = operands.into_inner();
                    let regs: Vec<Register> = registers
                        .by_ref()
                        .take(2)
                        .map(|p| Register::from_str(p.as_str().trim()))
                        .collect::<Result<Vec<_>, _>>()?;
                    let [rd, rs] = require_registes(regs, "I-type")?;
                    let imm_str = registers
                        .next()
                        .ok_or(AsmError::Missing { name: "Immediate" })?
                        .as_str()
                        .trim();
                    let imm: i32 = imm_str.parse()?;
                    Instruction::i_inst(op_code, rd, rs, imm)
                }
                Rule::binary_operand => {
                    let pseudo_code = PseudoCode::from_str(&op_token)?;
                    let mut registers = operands.into_inner();
                    let regs: Vec<Register> = registers
                        .by_ref()
                        .take(2)
                        .map(|p| Register::from_str(p.as_str().trim()))
                        .collect::<Result<Vec<_>, _>>()?;
                    let [rd, rs] = require_registes(regs, "Binary pseudo")?;
                    let op_code = match pseudo_code {
                        PseudoCode::MV => IOpCode::ADDI,
                        _ => todo!(),
                    };
                    Instruction::i_inst(op_code, rd, rs, 0)
                }
                Rule::binary_imm_operand => {
                    let pseudo_code = PseudoCode::from_str(&op_token)?;
                    let mut register = operands.into_inner();
                    let rd = Register::from_str(
                        register
                            .next()
                            .ok_or(AsmError::Missing { name: "Register" })?
                            .as_str(),
                    )?;
                    let imm_str = register
                        .next()
                        .ok_or(AsmError::Missing { name: "Immediate" })?
                        .as_str()
                        .trim();
                    let imm: i32 = imm_str.parse()?;
                    let op_code = match pseudo_code {
                        PseudoCode::LI => IOpCode::ADDI,
                        _ => todo!(),
                    };
                    Instruction::i_inst(op_code, rd, Register::Zero, imm)
                }
                _ => todo!(),
            };
            Ok(inst)
        } else {
            let pseudo_code = PseudoCode::from_str(&op_token)?;
            let inst = match pseudo_code {
                PseudoCode::RET => Instruction::ret(),
                _ => unreachable!(),
            };
            Ok(inst)
        }
    }
}

impl Assembler {
    pub fn iter(&mut self) -> std::slice::IterMut<'_, Instruction> {
        self.program.iter_mut()
    }

    pub fn find_label(&mut self, label: String) -> Result<u64, AsmError> {
        self.labels
            .get(&label)
            .copied()
            .ok_or(AsmError::UnknownLabel { msg: label })
    }

    pub fn to_json(&mut self) {
        let json = serde_json::to_string_pretty(&self.program).unwrap();
        println!("{}", json);
        let labels = serde_json::to_string_pretty(&self.labels).unwrap();
        println!("{}", labels);
    }
}
