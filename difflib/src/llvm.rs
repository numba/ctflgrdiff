use std::borrow::Cow;

impl crate::Program for llvm_ir::Module {
    const GAP: i32 = 2;

    type ParseOptions = bool;

    type ParseError = String;

    type Function = llvm_ir::Function;

    fn parse(
        file: impl AsRef<std::path::Path>,
        options: Self::ParseOptions,
    ) -> Result<Self, Self::ParseError> {
        // TODO: support LL files when available
        llvm_ir::Module::from_bc_path(file)
    }

    fn get(&self, name: &str) -> Option<&Self::Function> {
        self.get_func_by_name(name)
    }

    fn functions<'a>(&'a self) -> Box<dyn Iterator<Item = &'a Self::Function> + 'a> {
        Box::new(self.functions.iter())
    }
}

impl crate::Function for llvm_ir::Function {
    type BasicBlock = llvm_ir::BasicBlock;

    fn blocks<'a>(&'a self) -> Box<dyn Iterator<Item = &'a Self::BasicBlock> + 'a> {
        Box::new(self.basic_blocks.iter())
    }

    fn name<'a>(&'a self) -> std::borrow::Cow<'a, str> {
        Cow::Borrowed(self.name.as_str())
    }
}
impl crate::BasicBlock for llvm_ir::BasicBlock {
    type Instruction = llvm_ir::Instruction;
    type Terminator = llvm_ir::Terminator;

    fn get(&self, index: usize) -> &Self::Instruction {
        &self.instrs[index]
    }

    fn len(&self) -> usize {
        self.instrs.len()
    }

    fn name<'a>(&'a self) -> Cow<'a, str> {
        match &self.name {
            llvm_ir::Name::Name(n) => Cow::Borrowed(n),
            llvm_ir::Name::Number(n) => Cow::Owned(format!("%{}", n)),
        }
    }

    fn terminator(&self) -> &Self::Terminator {
        &self.term
    }
}
impl crate::Instruction for llvm_ir::Instruction {
    const EQUIVALENT: i32 = 4;
    fn score(&self, other: &Self) -> i32 {
        match (self, other) {
            (llvm_ir::Instruction::Add(_), llvm_ir::Instruction::Add(_))
            | (llvm_ir::Instruction::Sub(_), llvm_ir::Instruction::Sub(_))
            | (llvm_ir::Instruction::UDiv(_), llvm_ir::Instruction::UDiv(_))
            | (llvm_ir::Instruction::SDiv(_), llvm_ir::Instruction::SDiv(_))
            | (llvm_ir::Instruction::URem(_), llvm_ir::Instruction::URem(_))
            | (llvm_ir::Instruction::SRem(_), llvm_ir::Instruction::SRem(_)) => 4,
            (
                llvm_ir::Instruction::Add(_)
                | llvm_ir::Instruction::Sub(_)
                | llvm_ir::Instruction::UDiv(_)
                | llvm_ir::Instruction::SDiv(_)
                | llvm_ir::Instruction::URem(_)
                | llvm_ir::Instruction::SRem(_),
                llvm_ir::Instruction::Add(_)
                | llvm_ir::Instruction::Sub(_)
                | llvm_ir::Instruction::UDiv(_)
                | llvm_ir::Instruction::SDiv(_)
                | llvm_ir::Instruction::URem(_)
                | llvm_ir::Instruction::SRem(_),
            ) => 3,

            (llvm_ir::Instruction::And(_), llvm_ir::Instruction::And(_))
            | (llvm_ir::Instruction::Or(_), llvm_ir::Instruction::Or(_))
            | (llvm_ir::Instruction::Xor(_), llvm_ir::Instruction::Xor(_))
            | (llvm_ir::Instruction::Shl(_), llvm_ir::Instruction::Shl(_))
            | (llvm_ir::Instruction::LShr(_), llvm_ir::Instruction::LShr(_))
            | (llvm_ir::Instruction::AShr(_), llvm_ir::Instruction::AShr(_)) => 4,
            (
                llvm_ir::Instruction::And(_)
                | llvm_ir::Instruction::Or(_)
                | llvm_ir::Instruction::Xor(_)
                | llvm_ir::Instruction::Shl(_)
                | llvm_ir::Instruction::LShr(_)
                | llvm_ir::Instruction::AShr(_),
                llvm_ir::Instruction::And(_)
                | llvm_ir::Instruction::Or(_)
                | llvm_ir::Instruction::Xor(_)
                | llvm_ir::Instruction::Shl(_)
                | llvm_ir::Instruction::LShr(_)
                | llvm_ir::Instruction::AShr(_),
            ) => 3,

            // Floating-point ops
            (llvm_ir::Instruction::FAdd(_), llvm_ir::Instruction::FAdd(_))
            | (llvm_ir::Instruction::FSub(_), llvm_ir::Instruction::FSub(_))
            | (llvm_ir::Instruction::FMul(_), llvm_ir::Instruction::FMul(_))
            | (llvm_ir::Instruction::FDiv(_), llvm_ir::Instruction::FDiv(_))
            | (llvm_ir::Instruction::FRem(_), llvm_ir::Instruction::FRem(_))
            | (llvm_ir::Instruction::FNeg(_), llvm_ir::Instruction::FNeg(_)) => 4,
            (
                llvm_ir::Instruction::FAdd(_)
                | llvm_ir::Instruction::FSub(_)
                | llvm_ir::Instruction::FMul(_)
                | llvm_ir::Instruction::FDiv(_)
                | llvm_ir::Instruction::FRem(_)
                | llvm_ir::Instruction::FNeg(_),
                llvm_ir::Instruction::FAdd(_)
                | llvm_ir::Instruction::FSub(_)
                | llvm_ir::Instruction::FMul(_)
                | llvm_ir::Instruction::FDiv(_)
                | llvm_ir::Instruction::FRem(_)
                | llvm_ir::Instruction::FNeg(_),
            ) => 3,

            // Vector ops
            (llvm_ir::Instruction::ExtractElement(_), llvm_ir::Instruction::ExtractElement(_)) => 4,
            (llvm_ir::Instruction::InsertElement(_), llvm_ir::Instruction::InsertElement(_)) => 4,
            (llvm_ir::Instruction::ShuffleVector(_), llvm_ir::Instruction::ShuffleVector(_)) => 4,
            (llvm_ir::Instruction::Alloca(l), llvm_ir::Instruction::Alloca(r)) => {
                if l.allocated_type == r.allocated_type {
                    4
                } else {
                    3
                }
            }
            (llvm_ir::Instruction::Load(_), llvm_ir::Instruction::Load(_)) => 4,
            (llvm_ir::Instruction::Store(_), llvm_ir::Instruction::Store(_)) => 4,
            (llvm_ir::Instruction::Fence(l), llvm_ir::Instruction::Fence(r)) => {
                if l.atomicity == r.atomicity {
                    4
                } else {
                    3
                }
            }
            (llvm_ir::Instruction::CmpXchg(l), llvm_ir::Instruction::CmpXchg(r)) => {
                if l.atomicity == r.atomicity {
                    4
                } else {
                    3
                }
            }
            (llvm_ir::Instruction::AtomicRMW(l), llvm_ir::Instruction::AtomicRMW(r)) => {
                (if l.atomicity == r.atomicity { 2 } else { 1 })
                    + (if l.operation == r.operation { 4 } else { 3 })
            }
            (llvm_ir::Instruction::GetElementPtr(_), llvm_ir::Instruction::GetElementPtr(_)) => 4,

            // Conversion ops
            (llvm_ir::Instruction::Trunc(l), llvm_ir::Instruction::Trunc(r)) => {
                if l.to_type == r.to_type {
                    4
                } else {
                    3
                }
            }
            (llvm_ir::Instruction::ZExt(l), llvm_ir::Instruction::ZExt(r)) => {
                if l.to_type == r.to_type {
                    4
                } else {
                    3
                }
            }
            (llvm_ir::Instruction::SExt(l), llvm_ir::Instruction::SExt(r)) => {
                if l.to_type == r.to_type {
                    4
                } else {
                    3
                }
            }
            (llvm_ir::Instruction::ZExt(l), llvm_ir::Instruction::SExt(r)) => {
                if l.to_type == r.to_type {
                    3
                } else {
                    2
                }
            }
            (llvm_ir::Instruction::SExt(l), llvm_ir::Instruction::ZExt(r)) => {
                if l.to_type == r.to_type {
                    3
                } else {
                    2
                }
            }

            (llvm_ir::Instruction::FPTrunc(l), llvm_ir::Instruction::FPTrunc(r)) => {
                if l.to_type == r.to_type {
                    4
                } else {
                    3
                }
            }
            (llvm_ir::Instruction::FPExt(l), llvm_ir::Instruction::FPExt(r)) => {
                if l.to_type == r.to_type {
                    4
                } else {
                    3
                }
            }
            (llvm_ir::Instruction::FPTrunc(l), llvm_ir::Instruction::FPExt(r)) => {
                if l.to_type == r.to_type {
                    3
                } else {
                    2
                }
            }
            (llvm_ir::Instruction::FPExt(l), llvm_ir::Instruction::FPTrunc(r)) => {
                if l.to_type == r.to_type {
                    3
                } else {
                    2
                }
            }

            (llvm_ir::Instruction::FPToUI(l), llvm_ir::Instruction::FPToUI(r)) => {
                if l.to_type == r.to_type {
                    4
                } else {
                    3
                }
            }
            (llvm_ir::Instruction::FPToSI(l), llvm_ir::Instruction::FPToSI(r)) => {
                if l.to_type == r.to_type {
                    4
                } else {
                    3
                }
            }
            (llvm_ir::Instruction::FPToUI(l), llvm_ir::Instruction::FPToSI(r)) => {
                if l.to_type == r.to_type {
                    3
                } else {
                    2
                }
            }
            (llvm_ir::Instruction::FPToSI(l), llvm_ir::Instruction::FPToUI(r)) => {
                if l.to_type == r.to_type {
                    3
                } else {
                    2
                }
            }
            (llvm_ir::Instruction::UIToFP(l), llvm_ir::Instruction::UIToFP(r)) => {
                if l.to_type == r.to_type {
                    4
                } else {
                    3
                }
            }
            (llvm_ir::Instruction::SIToFP(l), llvm_ir::Instruction::SIToFP(r)) => {
                if l.to_type == r.to_type {
                    4
                } else {
                    3
                }
            }
            (llvm_ir::Instruction::UIToFP(l), llvm_ir::Instruction::SIToFP(r)) => {
                if l.to_type == r.to_type {
                    3
                } else {
                    2
                }
            }
            (llvm_ir::Instruction::SIToFP(l), llvm_ir::Instruction::UIToFP(r)) => {
                if l.to_type == r.to_type {
                    3
                } else {
                    2
                }
            }
            (llvm_ir::Instruction::PtrToInt(l), llvm_ir::Instruction::PtrToInt(r)) => {
                if l.to_type == r.to_type {
                    4
                } else {
                    3
                }
            }
            (llvm_ir::Instruction::IntToPtr(l), llvm_ir::Instruction::IntToPtr(r)) => {
                if l.to_type == r.to_type {
                    4
                } else {
                    3
                }
            }
            (llvm_ir::Instruction::BitCast(l), llvm_ir::Instruction::BitCast(r)) => {
                if l.to_type == r.to_type {
                    4
                } else {
                    3
                }
            }
            (llvm_ir::Instruction::AddrSpaceCast(l), llvm_ir::Instruction::AddrSpaceCast(r)) => {
                if l.to_type == r.to_type {
                    4
                } else {
                    3
                }
            }

            (llvm_ir::Instruction::ICmp(l), llvm_ir::Instruction::ICmp(r)) => {
                if l.predicate == r.predicate {
                    4
                } else {
                    3
                }
            }
            (llvm_ir::Instruction::FCmp(l), llvm_ir::Instruction::FCmp(r)) => {
                if l.predicate == r.predicate {
                    4
                } else {
                    3
                }
            }
            (llvm_ir::Instruction::Phi(l), llvm_ir::Instruction::Phi(r)) => {
                if l.to_type == r.to_type {
                    4
                } else {
                    3
                }
            }
            (llvm_ir::Instruction::Select(_), llvm_ir::Instruction::Select(_)) => 4,
            (llvm_ir::Instruction::Freeze(_), llvm_ir::Instruction::Freeze(_)) => 4,
            (llvm_ir::Instruction::Call(l), llvm_ir::Instruction::Call(r)) => {
                if l.arguments.len() == r.arguments.len() {
                    4
                } else {
                    3
                }
            }
            (llvm_ir::Instruction::VAArg(l), llvm_ir::Instruction::VAArg(r)) => {
                if l.cur_type == r.cur_type {
                    4
                } else {
                    3
                }
            }
            (llvm_ir::Instruction::LandingPad(_), llvm_ir::Instruction::LandingPad(_)) => 4,
            (llvm_ir::Instruction::CatchPad(_), llvm_ir::Instruction::CatchPad(_)) => 4,
            (llvm_ir::Instruction::CleanupPad(_), llvm_ir::Instruction::CleanupPad(_)) => 4,
            _ => 0,
        }
    }
    fn render<'a>(&self) -> Cow<'a, str> {
        Cow::Owned(self.to_string())
    }
}

impl crate::Instruction for llvm_ir::Terminator {
    const EQUIVALENT: i32 = 4;
    fn score(&self, other: &Self) -> i32 {
        match (self, other) {
            (llvm_ir::Terminator::Ret(_), llvm_ir::Terminator::Ret(_)) => 4,
            (llvm_ir::Terminator::Br(_), llvm_ir::Terminator::Br(_)) => 4,
            (llvm_ir::Terminator::CondBr(_), llvm_ir::Terminator::CondBr(_)) => 4,
            (llvm_ir::Terminator::Switch(_), llvm_ir::Terminator::Switch(_)) => 4,
            (llvm_ir::Terminator::IndirectBr(_), llvm_ir::Terminator::IndirectBr(_)) => 4,
            (llvm_ir::Terminator::Invoke(_), llvm_ir::Terminator::Invoke(_)) => 4,
            (llvm_ir::Terminator::Resume(_), llvm_ir::Terminator::Resume(_)) => 4,
            (llvm_ir::Terminator::Unreachable(_), llvm_ir::Terminator::Unreachable(_)) => 4,
            (llvm_ir::Terminator::CleanupRet(_), llvm_ir::Terminator::CleanupRet(_)) => 4,
            (llvm_ir::Terminator::CatchRet(_), llvm_ir::Terminator::CatchRet(_)) => 4,
            (llvm_ir::Terminator::CatchSwitch(_), llvm_ir::Terminator::CatchSwitch(_)) => 4,
            (llvm_ir::Terminator::CallBr(_), llvm_ir::Terminator::CallBr(_)) => 4,
            _ => 0,
        }
    }

    fn render<'a>(&self) -> Cow<'a, str> {
        Cow::Owned(self.to_string())
    }
}
