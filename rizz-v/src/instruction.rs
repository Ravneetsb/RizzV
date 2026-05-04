use crate::reg::*;
use serde::Serialize;

#[derive(Debug, Eq, PartialEq, Clone, Serialize, Default)]
pub enum Instruction {
    R(RInst),
    I(IInst),
    L(LInst),
    S(SInst),
    B(BInst),
    J(JInst),
    #[default]
    Nil,
}

impl Instruction {
    pub fn r_inst(op_code: ROpCode, rd: Register, rs1: Register, rs2: Register) -> Self {
        Self::R(RInst {
            op_code,
            rd,
            rs1,
            rs2,
        })
    }

    pub fn i_inst(op_code: IOpCode, rd: Register, rs: Register, imm: i32) -> Self {
        Self::I(IInst {
            op_code,
            rd,
            rs,
            imm,
        })
    }

    pub fn load_inst(op_code: LoadOpCode, rd: Register, address: MemoryRef) -> Self {
        Self::L(LInst {
            op_code,
            rd,
            address,
        })
    }

    pub fn store_inst(op_code: StoreOpCode, rs: Register, address: MemoryRef) -> Self {
        Self::S(SInst {
            op_code,
            rs,
            address,
        })
    }

    pub fn b_inst(op_code: BOpCode, rs1: Register, rs2: Register, target: Target) -> Self {
        Self::B(BInst {
            op_code,
            rs1,
            rs2,
            target,
        })
    }

    pub fn jal(rd: Register, target: Target) -> Self {
        Self::J(JInst {
            op_code: JOpCode::JAL,
            rd,
            target: JumpTarget::Direct(target),
        })
    }

    pub fn ret() -> Self {
        Self::J(JInst {
            op_code: JOpCode::JALR,
            rd: Register::Zero,
            target: JumpTarget::Indirect(MemoryRef {
                register: Register::RA,
                offset: 0,
            }),
        })
    }

    pub fn nil() -> Self {
        Self::Nil
    }

    pub fn is_control_flow(&self) -> bool {
        matches!(self, Self::B(_) | Self::J(_))
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

#[derive(Debug, Eq, PartialEq, Serialize, Clone)]
pub struct LInst {
    pub op_code: LoadOpCode,
    pub rd: Register,
    pub address: MemoryRef,
}

#[derive(Debug, Eq, PartialEq, Serialize, Clone)]
pub struct SInst {
    pub op_code: StoreOpCode,
    pub rs: Register,
    pub address: MemoryRef,
}

#[derive(Debug, Eq, PartialEq, Serialize, Clone)]
pub struct BInst {
    pub op_code: BOpCode,
    pub rs1: Register,
    pub rs2: Register,
    pub target: Target,
}

#[derive(Debug, Eq, PartialEq, Serialize, Clone)]
pub struct JInst {
    pub op_code: JOpCode,
    pub rd: Register,
    pub target: JumpTarget,
}

#[derive(Debug, Eq, PartialEq, Serialize, Clone)]
pub struct MemoryRef {
    pub register: Register,
    pub offset: i32,
}

#[derive(Debug, Eq, PartialEq, Serialize, Clone)]
pub enum JumpTarget {
    Direct(Target),
    Indirect(MemoryRef),
}

#[derive(Debug, Eq, PartialEq, Serialize, Clone)]
pub enum Target {
    Label(String),
    Address(u64),
}

impl Target {
    pub fn address(&self) -> Option<u64> {
        match self {
            Self::Address(value) => Some(*value),
            Self::Label(_) => None,
        }
    }
}
