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

impl Assembler {
    pub fn assemble(&mut self, pair: Pair<Rule>) {
        match pair.as_rule() {
            Rule::instruction => {
                let inst = Assembler::create_inst(pair);
                self.program.push(inst);
                self.pc += 4;
            }
            Rule::directive => {
                let mut inner = pair.into_inner();
                let directive = Directive::from_str(inner.next().unwrap().as_str()).unwrap();
                let label = inner.next().unwrap().as_str();
                match directive {
                    Directive::GLOBAL => {
                        self.globals.insert(label.to_string());
                    }
                }
            }
            Rule::label => {
                let mut inner = pair.into_inner();
                let lbl = inner.next().unwrap().as_str().trim().to_string();
                self.labels.insert(lbl, self.pc);
            }
            // _ => todo!("assembler"),
            _ => {
                for inner in pair.into_inner() {
                    self.assemble(inner);
                }
            }
        }
    }

    fn create_inst(pair: Pair<Rule>) -> Instruction {
        let mut inner = pair.into_inner();
        let op_token = inner
            .next()
            .expect("Expected Op Token found in instruction!")
            .as_str()
            .trim()
            .to_string();
        let ops = inner
            .next()
            .and_then(|operands| operands.into_inner().next());

        if let Some(operands) = ops {
            let inst = match operands.as_rule() {
                Rule::r_operand => {
                    let op_code = ROpCode::from_str(&op_token).unwrap();
                    let mut registers = operands.into_inner();
                    let regs: Vec<Register> = registers
                        .by_ref()
                        .take(3)
                        .map(|p| {
                            Register::from_str(p.as_str().trim())
                                .expect("Invalid register in R-type Inst")
                        })
                        .collect();

                    let [rd, rs1, rs2] = regs.try_into().expect("Invalid register in R-type Inst");
                    Instruction::r_inst(op_code, rd, rs1, rs2)
                }
                Rule::i_operand => {
                    let op_code = IOpCode::from_str(&op_token).expect("Invalid I-type Inst");
                    let mut registers = operands.into_inner();
                    let regs: Vec<Register> = registers
                        .by_ref()
                        .take(2)
                        .map(|p| {
                            Register::from_str(p.as_str().trim())
                                .expect("Invalid register in I-type Inst")
                        })
                        .collect();
                    let [rd, rs] = regs.try_into().unwrap();
                    let imm_str = registers.next().unwrap().as_str().trim();
                    let imm: i32 = imm_str.parse().expect("Invalid immediate in I-type Inst");
                    Instruction::i_inst(op_code, rd, rs, imm)
                }
                Rule::binary_operand => {
                    let pseudo_code =
                        PseudoCode::from_str(&op_token).expect("Invalid pseudo-instruction");
                    let mut registers = operands.into_inner();
                    let regs: Vec<Register> = registers
                        .by_ref()
                        .take(2)
                        .map(|p| {
                            Register::from_str(p.as_str().trim())
                                .expect("invalid reg in pseudo-instruction")
                        })
                        .collect();
                    let [rd, rs] = regs.try_into().unwrap();
                    let op_code = match pseudo_code {
                        PseudoCode::MV => IOpCode::ADDI,
                        _ => todo!(),
                    };
                    Instruction::i_inst(op_code, rd, rs, 0)
                }
                Rule::binary_imm_operand => {
                    let pseudo_code =
                        PseudoCode::from_str(&op_token).expect("Invalid pseudo-instruction");
                    let mut register = operands.into_inner();
                    let rd = Register::from_str(register.next().unwrap().as_str())
                        .expect("Invalid reg in pseudo-instruction");
                    let imm_str = register.next().unwrap().as_str().trim();
                    let imm: i32 = imm_str.parse().expect("Invalid immediate in I-type Inst");
                    let op_code = match pseudo_code {
                        PseudoCode::LI => IOpCode::ADDI,
                        _ => todo!(),
                    };
                    Instruction::i_inst(op_code, rd, Register::Zero, imm)
                }
                _ => todo!(),
            };
            inst
        } else {
            let pseudo_code =
                PseudoCode::from_str(&op_token).expect("Unknown op_code in pseudoinstruction");
            let inst = match pseudo_code {
                PseudoCode::RET => Instruction::ret(),
                _ => unreachable!(),
            };
            inst
        }
    }
}

impl Assembler {
    pub fn iter(&mut self) -> std::slice::IterMut<'_, Instruction> {
        self.program.iter_mut()
    }

    pub fn find_label(&mut self, label: String) -> u64 {
        *self.labels.get(&label).expect("Label not found")
    }

    pub fn to_json(&mut self) {
        let json = serde_json::to_string_pretty(&self.program).unwrap();
        println!("{}", json);
        let labels = serde_json::to_string_pretty(&self.labels).unwrap();
        println!("{}", labels);
    }
}
