use std::borrow::Cow;

use yaxpeax_x86::protected_mode::Opcode;
use yaxpeax_x86::x86_32;
impl super::MachArch for x86_32 {
    const CPU_TYPE: Option<u32> = Some(0x01000007);
}
impl super::YaxInstruction for yaxpeax_x86::protected_mode::Instruction {
    const GAP: i32 = -1;

    fn is_flow_control(&self) -> bool {
        match self.opcode() {
            Opcode::JA
            | Opcode::JB
            | Opcode::JECXZ
            | Opcode::JG
            | Opcode::JGE
            | Opcode::JL
            | Opcode::JLE
            | Opcode::JMPE
            | Opcode::JMPF
            | Opcode::JNA
            | Opcode::JNB
            | Opcode::JNO
            | Opcode::JNP
            | Opcode::JNS
            | Opcode::JNZ
            | Opcode::JS
            | Opcode::JZ
            | Opcode::RETURN
            | Opcode::RETF => true,
            _ => false,
        }
    }
}
impl crate::Instruction for yaxpeax_x86::protected_mode::Instruction {
    const EQUIVALENT: i32 = 4;
    fn score(&self, other: &Self) -> i32 {
        if self.opcode() == other.opcode() {
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
