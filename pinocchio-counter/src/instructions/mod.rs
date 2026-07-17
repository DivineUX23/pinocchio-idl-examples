pub mod initialize;
pub mod increment;
pub mod decrement;

pub use initialize::*;
pub use increment::*;
pub use decrement::*;

use pinocchio::error::ProgramError;

pub enum CounterInstruction {
    Initialize = 0,
    Increment  = 1,
    Decrement  = 2,
}

impl TryFrom<&u8> for CounterInstruction {
    type Error = ProgramError;

    fn try_from(value: &u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(CounterInstruction::Initialize),
            1 => Ok(CounterInstruction::Increment),
            2 => Ok(CounterInstruction::Decrement),
            _ => Err(ProgramError::InvalidInstructionData),
        }
    }
}
