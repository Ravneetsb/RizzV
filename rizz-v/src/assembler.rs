use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::num::ParseIntError;
use std::str::FromStr;

use derive_more::Display as MoreDisplay;
use pest::iterators::Pair;
use serde::Serialize;
use strum_macros::{Display, EnumIter, EnumString};

use crate::instruction::{Instruction, JumpTarget, MemoryRef, Target};
use crate::reg::{
    BOpCode, IOpCode, JOpCode, LoadOpCode, PseudoCode, ROpCode, Register, StoreOpCode,
};
use crate::Rule;

const WORD_SIZE: u64 = 4;

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

#[derive(Debug, Serialize)]
pub struct AssembledProgram<'a> {
    pub entry_pc: u64,
    pub labels: &'a HashMap<String, u64>,
    pub globals: Vec<&'a str>,
    pub instructions: &'a [Instruction],
}

fn require_registers<const N: usize>(
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

fn parse_register(raw: &str) -> Result<Register, AsmError> {
    Register::from_str(raw.trim()).map_err(|_| AsmError::Parse {
        msg: format!("unknown register: {}", raw.trim()),
    })
}

fn parse_r_opcode(raw: &str) -> Result<ROpCode, AsmError> {
    ROpCode::from_str(raw).map_err(|_| AsmError::Parse {
        msg: format!("unknown R opcode: {raw}"),
    })
}

fn parse_i_opcode(raw: &str) -> Result<IOpCode, AsmError> {
    IOpCode::from_str(raw).map_err(|_| AsmError::Parse {
        msg: format!("unknown I opcode: {raw}"),
    })
}

fn parse_load_opcode(raw: &str) -> Result<LoadOpCode, AsmError> {
    LoadOpCode::from_str(raw).map_err(|_| AsmError::Parse {
        msg: format!("unknown load opcode: {raw}"),
    })
}

fn parse_store_opcode(raw: &str) -> Result<StoreOpCode, AsmError> {
    StoreOpCode::from_str(raw).map_err(|_| AsmError::Parse {
        msg: format!("unknown store opcode: {raw}"),
    })
}

fn parse_b_opcode(raw: &str) -> Result<BOpCode, AsmError> {
    BOpCode::from_str(raw).map_err(|_| AsmError::Parse {
        msg: format!("unknown B opcode: {raw}"),
    })
}

fn parse_j_opcode(raw: &str) -> Result<JOpCode, AsmError> {
    JOpCode::from_str(raw).map_err(|_| AsmError::Parse {
        msg: format!("unknown J opcode: {raw}"),
    })
}

fn parse_pseudo(raw: &str) -> Result<PseudoCode, AsmError> {
    PseudoCode::from_str(raw).map_err(|_| AsmError::Parse {
        msg: format!("unknown pseudo-instruction: {raw}"),
    })
}

fn parse_memory_ref(
    mut parts: pest::iterators::Pairs<'_, Rule>,
) -> Result<(Register, MemoryRef), AsmError> {
    let subject = parse_register(
        parts
            .next()
            .ok_or(AsmError::Missing { name: "Register" })?
            .as_str(),
    )?;
    let mut address = parts
        .next()
        .ok_or(AsmError::Missing {
            name: "Memory address",
        })?
        .into_inner();
    let first = address.next().ok_or(AsmError::Missing {
        name: "Memory address component",
    })?;
    let (offset, base) = if first.as_rule() == Rule::operand_imm {
        let offset = first.as_str().trim().parse()?;
        let base = parse_register(
            address
                .next()
                .ok_or(AsmError::Missing {
                    name: "Base register",
                })?
                .as_str(),
        )?;
        (offset, base)
    } else {
        (0, parse_register(first.as_str())?)
    };
    Ok((
        subject,
        MemoryRef {
            register: base,
            offset,
        },
    ))
}

fn lower_two_register_branch(
    op_token: &str,
    rs1: Register,
    rs2: Register,
    label: String,
) -> Result<Instruction, AsmError> {
    let target = Target::Label(label);
    let inst = match op_token {
        "bgt" => Instruction::b_inst(BOpCode::BLT, rs2, rs1, target),
        "ble" => Instruction::b_inst(BOpCode::BGE, rs2, rs1, target),
        "bgtu" => Instruction::b_inst(BOpCode::BLTU, rs2, rs1, target),
        "bleu" => Instruction::b_inst(BOpCode::BGEU, rs2, rs1, target),
        _ => Instruction::b_inst(parse_b_opcode(op_token)?, rs1, rs2, target),
    };
    Ok(inst)
}

fn lower_zero_branch(op_token: &str, rs: Register, label: String) -> Result<Instruction, AsmError> {
    let target = Target::Label(label);
    let inst = match op_token {
        "beqz" => Instruction::b_inst(BOpCode::BEQ, rs, Register::Zero, target),
        "bnez" => Instruction::b_inst(BOpCode::BNE, rs, Register::Zero, target),
        "bltz" => Instruction::b_inst(BOpCode::BLT, rs, Register::Zero, target),
        "bgez" => Instruction::b_inst(BOpCode::BGE, rs, Register::Zero, target),
        _ => {
            return Err(AsmError::Parse {
                msg: format!("unknown zero-branch pseudo-instruction: {op_token}"),
            });
        }
    };
    Ok(inst)
}

impl Assembler {
    pub fn assemble(&mut self, pair: Pair<Rule>) -> Result<(), AsmError> {
        match pair.as_rule() {
            Rule::instruction => {
                let inst = Self::create_inst(pair)?;
                self.program.push(inst);
                self.pc += WORD_SIZE;
                Ok(())
            }
            Rule::directive => {
                let mut inner = pair.into_inner();
                let directive = Directive::from_str(
                    inner
                        .next()
                        .ok_or(AsmError::Missing { name: "directive" })?
                        .as_str(),
                )
                .map_err(|_| AsmError::Parse {
                    msg: "unknown directive".into(),
                })?;
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
                let label = inner
                    .next()
                    .ok_or(AsmError::Missing { name: "Label" })?
                    .as_str()
                    .trim()
                    .to_string();
                self.labels.insert(label, self.pc);
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
            .to_lowercase();

        let operands = inner.next().and_then(|pair| pair.into_inner().next());

        if let Some(operands) = operands {
            let inst = match operands.as_rule() {
                Rule::memory_operand => {
                    let (subject, mem_ref) = parse_memory_ref(operands.into_inner())?;
                    if let Ok(op_code) = parse_load_opcode(&op_token) {
                        Instruction::load_inst(op_code, subject, mem_ref)
                    } else {
                        Instruction::store_inst(parse_store_opcode(&op_token)?, subject, mem_ref)
                    }
                }
                Rule::r_operand => {
                    let op_code = parse_r_opcode(&op_token)?;
                    let regs = operands
                        .into_inner()
                        .map(|p| parse_register(p.as_str()))
                        .collect::<Result<Vec<_>, _>>()?;
                    let [rd, rs1, rs2] = require_registers(regs, "R-type")?;
                    Instruction::r_inst(op_code, rd, rs1, rs2)
                }
                Rule::i_operand => {
                    let op_code = parse_i_opcode(&op_token)?;
                    let mut parts = operands.into_inner();
                    let regs = parts
                        .by_ref()
                        .take(2)
                        .map(|p| parse_register(p.as_str()))
                        .collect::<Result<Vec<_>, _>>()?;
                    let [rd, rs] = require_registers(regs, "I-type")?;
                    let imm: i32 = parts
                        .next()
                        .ok_or(AsmError::Missing { name: "Immediate" })?
                        .as_str()
                        .trim()
                        .parse()?;
                    Instruction::i_inst(op_code, rd, rs, imm)
                }
                Rule::branch_operand => {
                    let mut parts = operands.into_inner();
                    let regs = parts
                        .by_ref()
                        .take(2)
                        .map(|p| parse_register(p.as_str()))
                        .collect::<Result<Vec<_>, _>>()?;
                    let [rs1, rs2] = require_registers(regs, "B-type")?;
                    let label = parts
                        .next()
                        .ok_or(AsmError::Missing { name: "Label" })?
                        .as_str()
                        .trim()
                        .to_string();
                    lower_two_register_branch(&op_token, rs1, rs2, label)?
                }
                Rule::label_operand => {
                    let mut parts = operands.into_inner();
                    let first = parts.next().ok_or(AsmError::Missing {
                        name: "Jump target",
                    })?;
                    let second = parts.next();
                    if matches!(op_token.as_str(), "beqz" | "bnez" | "bltz" | "bgez") {
                        let rs = parse_register(first.as_str())?;
                        let label = second
                            .ok_or(AsmError::Missing { name: "Label" })?
                            .as_str()
                            .trim()
                            .to_string();
                        lower_zero_branch(&op_token, rs, label)?
                    } else {
                        let (rd, label) = match second {
                            Some(label) => (
                                parse_register(first.as_str())?,
                                label.as_str().trim().to_string(),
                            ),
                            None => (Register::RA, first.as_str().trim().to_string()),
                        };

                        if op_token == "j" {
                            Instruction::jal(Register::Zero, Target::Label(label))
                        } else {
                            match parse_j_opcode(&op_token)? {
                                JOpCode::JAL => Instruction::jal(rd, Target::Label(label)),
                                JOpCode::JALR => {
                                    return Err(AsmError::Parse {
                                        msg: "jalr parsing is only supported via pseudo-instruction ret"
                                            .into(),
                                    });
                                }
                            }
                        }
                    }
                }
                Rule::binary_operand => {
                    let pseudo_code = parse_pseudo(&op_token)?;
                    let regs = operands
                        .into_inner()
                        .map(|p| parse_register(p.as_str()))
                        .collect::<Result<Vec<_>, _>>()?;
                    let [rd, rs] = require_registers(regs, "binary pseudo")?;
                    match pseudo_code {
                        PseudoCode::MV => Instruction::i_inst(IOpCode::ADDI, rd, rs, 0),
                        _ => {
                            return Err(AsmError::Parse {
                                msg: format!(
                                    "unsupported binary pseudo-instruction: {pseudo_code}"
                                ),
                            });
                        }
                    }
                }
                Rule::binary_imm_operand => {
                    let pseudo_code = parse_pseudo(&op_token)?;
                    let mut parts = operands.into_inner();
                    let rd = parse_register(
                        parts
                            .next()
                            .ok_or(AsmError::Missing { name: "Register" })?
                            .as_str(),
                    )?;
                    let imm: i32 = parts
                        .next()
                        .ok_or(AsmError::Missing { name: "Immediate" })?
                        .as_str()
                        .trim()
                        .parse()?;
                    match pseudo_code {
                        PseudoCode::LI => {
                            Instruction::i_inst(IOpCode::ADDI, rd, Register::Zero, imm)
                        }
                        _ => {
                            return Err(AsmError::Parse {
                                msg: format!(
                                    "unsupported immediate pseudo-instruction: {pseudo_code}"
                                ),
                            });
                        }
                    }
                }
                _ => {
                    return Err(AsmError::Parse {
                        msg: format!("unsupported operand pattern: {:?}", operands.as_rule()),
                    });
                }
            };
            Ok(inst)
        } else {
            let pseudo_code = parse_pseudo(&op_token)?;
            match pseudo_code {
                PseudoCode::RET => Ok(Instruction::ret()),
                _ => Err(AsmError::Parse {
                    msg: format!("unsupported zero-operand pseudo-instruction: {pseudo_code}"),
                }),
            }
        }
    }

    pub fn resolve(&mut self) -> Result<(), AsmError> {
        let labels = self.labels.clone();
        for instruction in &mut self.program {
            match instruction {
                Instruction::B(branch) => {
                    branch.target = resolve_target_with_labels(&labels, &branch.target)?;
                }
                Instruction::J(jump) => {
                    if let JumpTarget::Direct(target) = &jump.target {
                        jump.target =
                            JumpTarget::Direct(resolve_target_with_labels(&labels, target)?);
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }

    pub fn entry_pc(&self) -> u64 {
        self.globals
            .iter()
            .filter_map(|label| self.labels.get(label))
            .copied()
            .min()
            .unwrap_or(0)
    }

    pub fn entry_label(&self) -> Option<String> {
        let entry_pc = self.entry_pc();
        let mut labels = self
            .globals
            .iter()
            .filter_map(|label| self.labels.get(label).map(|pc| (label, *pc)))
            .collect::<Vec<_>>();
        labels.sort_by_key(|(_, pc)| *pc);
        labels
            .into_iter()
            .find(|(_, pc)| *pc == entry_pc)
            .map(|(label, _)| label.clone())
    }

    pub fn instructions(&self) -> &[Instruction] {
        &self.program
    }

    pub fn instruction_at_pc(&self, pc: u64) -> Option<&Instruction> {
        if !pc.is_multiple_of(WORD_SIZE) {
            return None;
        }
        self.program.get((pc / WORD_SIZE) as usize)
    }

    pub fn labels(&self) -> &HashMap<String, u64> {
        &self.labels
    }

    pub fn assembled(&self) -> AssembledProgram<'_> {
        let mut globals = self.globals.iter().map(String::as_str).collect::<Vec<_>>();
        globals.sort_unstable();
        AssembledProgram {
            entry_pc: self.entry_pc(),
            labels: &self.labels,
            globals,
            instructions: &self.program,
        }
    }

    pub fn to_json(&self) -> Result<String, AsmError> {
        serde_json::to_string_pretty(&self.assembled()).map_err(Into::into)
    }
}

fn resolve_target_with_labels(
    labels: &HashMap<String, u64>,
    target: &Target,
) -> Result<Target, AsmError> {
    match target {
        Target::Label(label) => labels
            .get(label)
            .copied()
            .map(Target::Address)
            .ok_or_else(|| AsmError::UnknownLabel { msg: label.clone() }),
        Target::Address(value) => Ok(Target::Address(*value)),
    }
}
