use std::collections::BTreeMap;

use derive_more::Display as MoreDisplay;
use serde::Serialize;

const STACK_BASE: u64 = 0x1000;
const STACK_SIZE: u64 = 0x1000;

#[derive(Debug, MoreDisplay)]
pub enum MemoryError {
    #[display("misaligned memory access for {name} at address {addr}")]
    MisalignedMemory { name: &'static str, addr: u64 },
    #[display("invalid memory address: {msg}")]
    InvalidMemAddr { msg: String },
}

#[derive(Debug, Eq, PartialEq, Clone, Serialize)]
pub enum MemoryEventKind {
    Load,
    Store,
}

#[derive(Debug, Eq, PartialEq, Clone, Serialize)]
pub struct MemoryEvent {
    pub kind: MemoryEventKind,
    pub address: u64,
    pub width: u8,
    pub value: i64,
    pub previous_value: Option<i64>,
}

#[derive(Debug, Default, PartialEq, Eq, Clone)]
pub struct Memory {
    bytes: BTreeMap<u64, u8>,
}

impl Memory {
    pub fn stack_base() -> u64 {
        STACK_BASE
    }

    pub fn stack_top() -> u64 {
        STACK_BASE + STACK_SIZE
    }

    pub fn initial_stack_pointer() -> u64 {
        Self::stack_top() - 8
    }

    pub fn load8(&self, addr: u64) -> Result<u8, MemoryError> {
        self.validate_address(addr, 1)?;
        Ok(*self.bytes.get(&addr).unwrap_or(&0))
    }

    pub fn load16(&self, addr: u64) -> Result<u16, MemoryError> {
        self.validate_alignment(addr, 2, "halfword load")?;
        self.validate_address(addr, 2)?;
        let lo = self.load8(addr)? as u16;
        let hi = self.load8(addr + 1)? as u16;
        Ok(lo | (hi << 8))
    }

    pub fn load32(&self, addr: u64) -> Result<u32, MemoryError> {
        self.validate_alignment(addr, 4, "word load")?;
        self.validate_address(addr, 4)?;
        let b0 = self.load8(addr)? as u32;
        let b1 = self.load8(addr + 1)? as u32;
        let b2 = self.load8(addr + 2)? as u32;
        let b3 = self.load8(addr + 3)? as u32;
        Ok(b0 | (b1 << 8) | (b2 << 16) | (b3 << 24))
    }

    pub fn load64(&self, addr: u64) -> Result<u64, MemoryError> {
        self.validate_alignment(addr, 8, "doubleword load")?;
        self.validate_address(addr, 8)?;
        let mut value = 0u64;
        for offset in 0..8 {
            value |= (self.load8(addr + offset)? as u64) << (offset * 8);
        }
        Ok(value)
    }

    pub fn store8(&mut self, addr: u64, value: u8) -> Result<i64, MemoryError> {
        self.validate_address(addr, 1)?;
        let previous = self.load8(addr)? as i8 as i64;
        self.bytes.insert(addr, value);
        Ok(previous)
    }

    pub fn store16(&mut self, addr: u64, value: u16) -> Result<i64, MemoryError> {
        self.validate_alignment(addr, 2, "halfword store")?;
        self.validate_address(addr, 2)?;
        let previous = self.load16(addr)? as i16 as i64;
        self.bytes.insert(addr, (value & 0xff) as u8);
        self.bytes.insert(addr + 1, ((value >> 8) & 0xff) as u8);
        Ok(previous)
    }

    pub fn store32(&mut self, addr: u64, value: u32) -> Result<i64, MemoryError> {
        self.validate_alignment(addr, 4, "word store")?;
        self.validate_address(addr, 4)?;
        let previous = self.load32(addr)? as i32 as i64;
        self.bytes.insert(addr, (value & 0xff) as u8);
        self.bytes.insert(addr + 1, ((value >> 8) & 0xff) as u8);
        self.bytes.insert(addr + 2, ((value >> 16) & 0xff) as u8);
        self.bytes.insert(addr + 3, ((value >> 24) & 0xff) as u8);
        Ok(previous)
    }

    pub fn store64(&mut self, addr: u64, value: u64) -> Result<i64, MemoryError> {
        self.validate_alignment(addr, 8, "doubleword store")?;
        self.validate_address(addr, 8)?;
        let previous = self.load64(addr)? as i64;
        for offset in 0..8 {
            self.bytes
                .insert(addr + offset, ((value >> (offset * 8)) & 0xff) as u8);
        }
        Ok(previous)
    }

    fn validate_alignment(
        &self,
        addr: u64,
        width: u64,
        name: &'static str,
    ) -> Result<(), MemoryError> {
        if !addr.is_multiple_of(width) {
            return Err(MemoryError::MisalignedMemory { name, addr });
        }
        Ok(())
    }

    fn validate_address(&self, addr: u64, width: u64) -> Result<(), MemoryError> {
        if width == 0 {
            return Ok(());
        }
        let Some(end) = addr.checked_add(width - 1) else {
            return Err(MemoryError::InvalidMemAddr {
                msg: format!("address overflow at {addr}"),
            });
        };
        if addr < STACK_BASE || end >= Self::stack_top() {
            return Err(MemoryError::InvalidMemAddr {
                msg: format!("address range {addr:#x}..={end:#x} is outside stack window"),
            });
        }
        Ok(())
    }
}
