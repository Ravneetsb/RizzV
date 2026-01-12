use crate::instruction::{IInst, Instruction, RInst};
use crate::reg::File;
use pest::{Parser, iterators::Pair};
use pest_derive::Parser;
use std::fs;
use std::str::FromStr;
pub mod assembler;
pub mod cpu;
pub mod executor;
pub mod instruction;
pub mod reg;
pub mod state;
use executor::Executor;
use reg::{IOpCode, ROpCode, RegFile, Register};
use serde_json;
use state::{State, States};

//  NOTE The correct way to update a map
// regs.entry(r).and_modify(|v| *v += 1).or_insert(1);

// TODO FIX COMMENT RULE
#[derive(Parser)]
#[grammar = "grammar.pest"]
pub struct RizzParser;

fn run(pair: Pair<Rule>, regs: &mut RegFile, states: &mut States) {
    match pair.as_rule() {
        Rule::label => {
            println!("Found a label!");
            let mut inner = pair.into_inner();
            let lbl = inner.next().unwrap().as_str().trim().to_string();
            println!("{}", lbl);
        }
        Rule::instruction => {
            let mut inner = pair.into_inner();
            let op_token = inner.next().unwrap().as_str().trim().to_string();
            let operands = inner.next().unwrap().into_inner().next().unwrap();
            // println!("{:?}", operands);
            match operands.as_rule() {
                Rule::r_operand => {
                    let op_code = ROpCode::from_str(&op_token).unwrap();
                    let mut registers = operands.into_inner();
                    // registers = registers.next().unwrap().into_inner();

                    let rd = Register::from_str(registers.next().unwrap().as_str()).unwrap();
                    let rs1 = Register::from_str(registers.next().unwrap().as_str()).unwrap();
                    let rs2 = Register::from_str(registers.next().unwrap().as_str())
                        .expect("Did not find the variant for rs2");
                    match op_code {
                        ROpCode::ADD => {
                            let sum = regs.get(rs1.clone()) + regs.get(rs2.clone());
                            regs.set(rd.clone(), sum);
                        }
                        ROpCode::SUB => {
                            let result = regs.get(rs1.clone()) - regs.get(rs2.clone());
                            regs.set(rd.clone(), result);
                        }
                        ROpCode::MUL => {
                            let result = regs.get(rs1.clone()) * regs.get(rs2.clone());
                            regs.set(rd.clone(), result);
                        }
                        ROpCode::DIV => {
                            let result = regs.get(rs1.clone()) / regs.get(rs2.clone());
                            regs.set(rd.clone(), result);
                        }
                        ROpCode::SLL => {
                            let result = regs.get(rs1.clone()) << regs.get(rs2.clone());
                            regs.set(rd.clone(), result);
                        }
                    }
                    let state = State {
                        step: states.count + 1,
                        reg_file: regs.clone(),
                        instr: Instruction::R(RInst {
                            op_code: op_code.clone(),
                            rd: rd.clone(),
                            rs1: rs1.clone(),
                            rs2: rs2.clone(),
                        }),
                    };
                    states.add(state);
                }
                Rule::i_operand => {
                    let op_code = IOpCode::from_str(&op_token).unwrap();
                    let mut it = operands.into_inner();
                    let rd = Register::from_str(it.next().unwrap().as_str()).unwrap();
                    let rs = Register::from_str(it.next().unwrap().as_str()).unwrap();
                    // println!("{:?}", it);
                    let imm_str = it.next().unwrap().as_str().trim();
                    let imm: i32 = imm_str.parse().unwrap();
                    match op_code {
                        IOpCode::ADDI => {
                            regs.set(rd.clone(), regs.get(rs.clone()) + imm as i64);

                            states.add(State {
                                step: states.count + 1,
                                reg_file: regs.clone(),
                                instr: Instruction::I(IInst {
                                    op_code,
                                    rd,
                                    rs,
                                    imm,
                                }),
                            });
                        }
                    }
                }
                Rule::binary_operand => {
                    todo!("mv")
                }
                Rule::binary_imm_operand => {
                    todo!("li")
                }
                _ => todo!(),
            }
        }
        _ => {
            for inner in pair.into_inner() {
                run(inner, regs, states);
            }
        }
    }
}

fn main() {
    let input = fs::read_to_string("itest.s").unwrap();
    let mut asm = assembler::Assembler::default();
    let pair = RizzParser::parse(Rule::program, &input)
        .expect("oof")
        .next()
        .unwrap();
    asm.assemble(pair).expect("Assembly failed");
    // asm.to_json();
    let mut executor = Executor::default();
    executor.execute(&mut asm);
}
