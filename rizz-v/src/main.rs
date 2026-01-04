use crate::instruction::Instruction;
use crate::reg::File;
use pest::{Parser, iterators::Pair};
use pest_derive::Parser;
use std::fs;
use std::str::FromStr;
pub mod instruction;
pub mod reg;
pub mod state;
use reg::{OpCode, RegFile, Register};
use state::{State, States};

//  NOTE The correct way to update a map
// regs.entry(r).and_modify(|v| *v += 1).or_insert(1);

// TODO FIX COMMENT RULE
#[derive(Parser)]
#[grammar = "grammar.pest"]
pub struct RizzParser;

fn run(pair: Pair<Rule>, regs: &mut RegFile, states: &mut States) {
    match pair.as_rule() {
        Rule::instruction => {
            let mut inner = pair.into_inner();
            let op_code = OpCode::from_str(inner.next().unwrap().as_str()).unwrap();
            let operands = inner.next().unwrap().into_inner().next().unwrap();
            let mut registers = operands.into_inner();

            let rd = Register::from_str(registers.next().unwrap().as_str()).unwrap();
            let rs1 = Register::from_str(registers.next().unwrap().as_str()).unwrap();
            let rs2 = Register::from_str(registers.next().unwrap().as_str())
                .expect("Did not find the variant for rs2");
            let state = State {
                step: states.count + 1,
                reg_file: regs.clone(),
                instr: Instruction {
                    op_code: op_code.clone(),
                    rd: rd.clone(),
                    rs1: rs1.clone(),
                    rs2: rs2.clone(),
                },
            };
            states.add(state);
            match op_code {
                OpCode::ADD => {
                    let sum = regs.get(rs1) + regs.get(rs2);
                    regs.set(rd, sum);
                }
                OpCode::SUB => {
                    let result = regs.get(rs1) - regs.get(rs2);
                    regs.set(rd, result);
                }
                OpCode::MUL => {
                    let result = regs.get(rs1) * regs.get(rs2);
                    regs.set(rd, result);
                }
                OpCode::DIV => {
                    let result = regs.get(rs1) / regs.get(rs2);
                    regs.set(rd, result);
                }
                OpCode::SLL => {
                    let result = regs.get(rs1) << regs.get(rs2);
                    regs.set(rd, result);
                }
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
    let mut regs = reg::RegFile::default();
    let input = fs::read_to_string("test.s").unwrap();
    let mut states = States::default();
    regs.set(Register::A0, 10);
    regs.set(Register::A1, 3);
    println!("{:?}", regs);

    let parsed = RizzParser::parse(Rule::program, &input)
        .expect("oof")
        .next()
        .unwrap();
    run(parsed, &mut regs, &mut states);
    println!("{:?}", regs);
    println!("{:?}", states);
}
