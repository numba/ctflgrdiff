use std::borrow::Cow;

use yaxpeax_arm::armv8::a64::Opcode;
impl super::MachArch for yaxpeax_arm::armv8::a64::ARMv8 {
    const CPU_TYPE: Option<u32> = Some(0x0100000c);
}
impl super::YaxInstruction for yaxpeax_arm::armv8::a64::Instruction {
    const GAP: i32 = -1;

    fn is_flow_control(&self) -> bool {
        match self.opcode {
            Opcode::B
            | Opcode::BCAX
            | Opcode::BL
            | Opcode::BLR
            | Opcode::Bcc(_)
            | Opcode::CBNZ
            | Opcode::CBZ
            | Opcode::RET
            | Opcode::TBNZ
            | Opcode::TBZ => true,
            _ => false,
        }
    }
}
impl crate::Instruction for yaxpeax_arm::armv8::a64::Instruction {
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
