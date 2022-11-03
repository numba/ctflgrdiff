use std::borrow::Cow;

use yaxpeax_avr::Opcode;
impl super::MachArch for yaxpeax_avr::AVR {
    const CPU_TYPE: Option<u32> = None;
}
impl super::YaxInstruction for yaxpeax_avr::Instruction {
    const GAP: i32 = -1;

    fn is_flow_control(&self) -> bool {
        match self.opcode {
            Opcode::EIJMP
            | Opcode::IJMP
            | Opcode::JMP
            | Opcode::RET
            | Opcode::RJMP
            | Opcode::RETI => true,
            _ => false,
        }
    }
}
impl crate::Instruction for yaxpeax_avr::Instruction {
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
