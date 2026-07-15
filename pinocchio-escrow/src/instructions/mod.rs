pub mod make;
pub mod take;
pub mod refund;

pub use make::*;
pub use take::*;
pub use refund::*;

use pinocchio::error::ProgramError;

pub enum EscrowInstruction {
    Make   = 0,
    Take   = 1,
    Refund = 2,
}

impl TryFrom<&u8> for EscrowInstruction {
    type Error = ProgramError;

    fn try_from(value: &u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(EscrowInstruction::Make),
            1 => Ok(EscrowInstruction::Take),
            2 => Ok(EscrowInstruction::Refund),
            _ => Err(ProgramError::InvalidInstructionData),
        }
    }
}
