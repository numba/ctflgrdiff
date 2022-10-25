use std::borrow::Cow;

use yaxpeax_arm::armv7::Opcode;
impl super::MachArch for yaxpeax_arm::armv7::ARMv7 {
    const CPU_TYPE: Option<u32> = Some(0x0000000c);
}
impl super::YaxInstruction for yaxpeax_arm::armv7::Instruction {
    const GAP: i32 = -1;

    fn is_flow_control(&self) -> bool {
        match self.opcode {
            Opcode::B
            | Opcode::BLX
            | Opcode::BX
            | Opcode::BXJ
            | Opcode::CBNZ
            | Opcode::CBZ
            | Opcode::BL
            | Opcode::TBB
            | Opcode::TBH => true,
            _ => false,
        }
    }
}
impl crate::Instruction for yaxpeax_arm::armv7::Instruction {
    const EQUIVALENT: i32 = 4;
    fn score(&self, other: &Self) -> i32 {
        if self.opcode == other.opcode {
            4
        } else {
            // TODO: come up with better scoring
            0
        }
    }

    fn render<'a>(&self) -> Cow<'a, str> {
        Cow::Owned(self.to_string())
    }
}
