use std::collections::HashMap;
use std::str::FromStr;

use pest::iterators::Pair;
use serde::Serialize;

use crate::Rule;
use crate::cpu::CPU;
use crate::instruction::{IInst, Instruction, JInst, Memory, RInst};
use crate::reg::{IOpCode, PseudoCode, ROpCode, RegFile, Register};
use serde_json;

#[derive(Debug, Default, PartialEq, Eq, Clone, Serialize)]
pub struct Assembler {
    program: Vec<Instruction>,
    cpu: CPU,
    labels: HashMap<String, u64>,
}

impl Assembler {
    pub fn assemble(&mut self, pair: Pair<Rule>) {
        match pair.as_rule() {
            Rule::instruction => {
                let inst = Assembler::create_inst(pair);
                self.program.push(inst);
                self.cpu.increment_byte();
            }
            Rule::label => {
                let mut inner = pair.into_inner();
                let lbl = inner.next().unwrap().as_str().trim().to_string();
                self.labels.insert(lbl, self.cpu.get_pc());
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
                            Register::from_str(p.as_str()).expect("Invalid register in R-type Inst")
                        })
                        .collect();

                    let [rd, rs1, rs2] = regs.try_into().expect("Invalid register in R-type Inst");
                    Instruction::R(RInst {
                        op_code,
                        rd,
                        rs1,
                        rs2,
                    })
                }
                Rule::i_operand => {
                    let op_code = IOpCode::from_str(&op_token).expect("Invalid I-type Inst");
                    let mut registers = operands.into_inner();
                    let regs: Vec<Register> = registers
                        .by_ref()
                        .take(2)
                        .map(|p| {
                            Register::from_str(p.as_str()).expect("Invalid register in I-type Inst")
                        })
                        .collect();
                    let [rd, rs] = regs.try_into().unwrap();
                    let imm_str = registers.next().unwrap().as_str().trim();
                    let imm: i32 = imm_str.parse().expect("Invalid immediate in I-type Inst");
                    Instruction::I(IInst {
                        op_code,
                        rd,
                        rs,
                        imm,
                    })
                }
                Rule::binary_operand => {
                    let pseudo_code =
                        PseudoCode::from_str(&op_token).expect("Invalid pseudo-instruction");
                    let mut registers = operands.into_inner();
                    let regs: Vec<Register> = registers
                        .by_ref()
                        .take(2)
                        .map(|p| {
                            Register::from_str(p.as_str())
                                .expect("invalid reg in pseudo-instruction")
                        })
                        .collect();
                    let [rd, rs] = regs.try_into().unwrap();
                    let op_code = match pseudo_code {
                        PseudoCode::MV => IOpCode::ADDI,
                        _ => todo!(),
                    };
                    Instruction::I(IInst {
                        op_code,
                        rd,
                        rs,
                        imm: 0,
                    })
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
                    Instruction::I(IInst {
                        op_code,
                        rd,
                        rs: Register::Zero,
                        imm,
                    })
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
    pub fn to_json(&mut self) {
        let json = serde_json::to_string_pretty(&self).unwrap();
        println!("{}", json);
    }
}
