pub mod arm32;
pub mod arm64;
pub mod avr;
pub mod x86_32;
pub mod x86_64;
use goblin::mach::constants::S_ATTR_PURE_INSTRUCTIONS;
use goblin::mach::constants::S_ATTR_SOME_INSTRUCTIONS;
use goblin::mach::symbols::NO_SECT;
use goblin::Object;
use num_traits::Zero;
use std::collections::BTreeMap;
use std::fmt::Display;
use yaxpeax_arch::AddressBase;
use yaxpeax_arch::Arch;
use yaxpeax_arch::Decoder;
use yaxpeax_arch::LengthedInstruction;
use yaxpeax_arch::Reader;
use yaxpeax_arch::U8Reader;
pub struct GoblinYax<A: yaxpeax_arch::Arch> {
    funcs: BTreeMap<String, GoblinYaxFunction<A>>,
}

pub struct GoblinYaxFunction<A: yaxpeax_arch::Arch> {
    blocks: Vec<GoblinYaxBlock<A>>,
    name: String,
}
pub struct GoblinYaxBlock<A: yaxpeax_arch::Arch> {
    id: usize,
    instructions: Vec<A::Instruction>,
    terminator: Option<A::Instruction>,
}
pub enum GoblinYaxError<A: yaxpeax_arch::Arch> {
    Fat,
    Goblin(goblin::error::Error),
    Io(std::io::Error),
    NoSym,
    Unrecognized,
    Yax(A::DecodeError),
}
impl<A: yaxpeax_arch::Arch> Display for GoblinYaxError<A> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GoblinYaxError::Fat => f.write_str("architecture is not present in FAT binary"),
            GoblinYaxError::Goblin(g) => g.fmt(f),
            GoblinYaxError::Io(i) => i.fmt(f),
            GoblinYaxError::NoSym => f.write_str("symbol table is corrupt"),
            GoblinYaxError::Unrecognized => f.write_str("cannot parse file"),
            GoblinYaxError::Yax(y) => y.fmt(f),
        }
    }
}

trait YaxInstruction: crate::Instruction {
    const GAP: i32;
    fn is_flow_control(&self) -> bool;
}

trait MachArch {
    const CPU_TYPE: Option<u32>;
}

impl<A: yaxpeax_arch::Arch + MachArch> crate::Program for GoblinYax<A>
where
    for<'a> U8Reader<'a>: Reader<<A as Arch>::Address, <A as Arch>::Word>,
    A::Instruction: YaxInstruction,
{
    const GAP: i32 = A::Instruction::GAP;

    type ParseError = GoblinYaxError<A>;

    type Function = GoblinYaxFunction<A>;

    fn parse(file: impl AsRef<std::path::Path>) -> Result<Self, Self::ParseError> {
        let buffer = std::fs::read(file).map_err(GoblinYaxError::Io)?;
        fn convert<'a, A: yaxpeax_arch::Arch + MachArch, S: AsRef<str>>(
            buffer: &[u8],
            iter: impl Iterator<Item = (Option<S>, usize, usize)>,
        ) -> Result<BTreeMap<String, GoblinYaxFunction<A>>, GoblinYaxError<A>>
        where
            for<'r> U8Reader<'r>: Reader<<A as Arch>::Address, <A as Arch>::Word>,
            A::Instruction: YaxInstruction,
        {
            iter.map(|(name, start, end)| {
                let name = demangle(name.ok_or(GoblinYaxError::NoSym)?.as_ref());
                let decoder = A::Decoder::default();
                let mut addr = A::Address::zero();
                let mut blocks = Vec::new();
                let mut instructions = Vec::new();
                while let Some(rest) = buffer
                    .get((start + addr.to_linear())..end)
                    .filter(|v| !v.is_empty())
                {
                    let mut reader = U8Reader::new(rest);
                    match decoder.decode(&mut reader) {
                        Ok(inst) => {
                            addr += inst.len();
                            let new_block = inst.is_flow_control();
                            if new_block {
                                let id = blocks.len();
                                blocks.push(GoblinYaxBlock {
                                    id,
                                    instructions,
                                    terminator: Some(inst),
                                });
                                instructions = Vec::new();
                            } else {
                                instructions.push(inst);
                            }
                        }
                        Err(e) => {
                            return Err(GoblinYaxError::Yax(e));
                        }
                    }
                }
                if !instructions.is_empty() {
                    // This means a chunk of assembly with no terminal flow control...
                    let id = blocks.len();
                    blocks.push(GoblinYaxBlock {
                        id,
                        instructions,
                        terminator: None,
                    });
                }
                Ok((name.clone(), GoblinYaxFunction::<A> { blocks, name }))
            })
            .collect()
        }
        fn extract<A: yaxpeax_arch::Arch + MachArch>(
            buffer: &[u8],
        ) -> Result<BTreeMap<String, GoblinYaxFunction<A>>, GoblinYaxError<A>>
        where
            for<'a> U8Reader<'a>: Reader<<A as Arch>::Address, <A as Arch>::Word>,
            A::Instruction: YaxInstruction,
        {
            Ok(
                match Object::parse(&buffer).map_err(GoblinYaxError::Goblin)? {
                    Object::Elf(elf) => convert(
                        buffer,
                        elf.dynsyms
                            .iter()
                            .map(|sym| (elf.dynstrtab.get_at(sym.st_name), sym))
                            .chain(
                                elf.syms
                                    .iter()
                                    .map(|sym| (elf.strtab.get_at(sym.st_name), sym)),
                            )
                            .filter(|(_, sym)| sym.is_function() && sym.st_size > 0)
                            .map(|(name, sym)| {
                                let section = &elf.section_headers[sym.st_shndx];
                                let start =
                                    (section.sh_offset + sym.st_value - section.sh_addr) as usize;
                                (name, start, (start + sym.st_size as usize))
                            }),
                    )?,
                    Object::PE(pe) => convert(
                        buffer,
                        pe.exports.iter().filter_map(|export| {
                            match (&export.name, export.offset) {
                                (Some(name), Some(start)) => {
                                    // PE doesn't include symbol lengths, so we try to find the next symbol and if we can't, we just assume it takes up the remainder of the section.
                                    let previous_symbol = pe
                                        .exports
                                        .iter()
                                        .flat_map(|other| other.offset.iter().copied())
                                        .filter(|&other| other > start)
                                        .min();
                                    match previous_symbol {
                                        Some(end) => Some((Some(name), start, end)),
                                        None => {
                                            eprintln!("Can't determine the size of {}.", name);
                                            None
                                        }
                                    }
                                }
                                _ => None,
                            }
                        }),
                    )?,
                    Object::Mach(mach) => {
                        let mach = match mach {
                            goblin::mach::Mach::Fat(fat) => match A::CPU_TYPE {
                                Some(cpu_type) => {
                                    let arch = fat
                                        .find_cputype(cpu_type)
                                        .map_err(GoblinYaxError::Goblin)?
                                        .ok_or(GoblinYaxError::Fat)?;
                                    goblin::mach::MachO::parse(&buffer, arch.offset as usize)
                                        .map_err(GoblinYaxError::Goblin)?
                                }
                                None => {
                                    return Err(GoblinYaxError::Fat);
                                }
                            },
                            goblin::mach::Mach::Binary(bin) => bin,
                        };
                        convert(
                            buffer,
                            mach.symbols()
                                .filter_map(|sym| match sym {
                                    Err(_) => None,
                                    Ok((name, list)) => {
                                        if list.n_sect == NO_SECT.into() {
                                            None
                                        } else {
                                            let mut section = list.n_sect - 1;
                                            let mut fileoff = 0;
                                            for segment in &mach.segments {
                                                if section < segment.nsects as usize {
                                                    let sections = segment.sections().ok()?;
                                                    let (section, _) = &sections[section];
                                                    if section.flags & S_ATTR_PURE_INSTRUCTIONS == 0
                                                        && section.flags & S_ATTR_SOME_INSTRUCTIONS
                                                            == 0
                                                    {
                                                        return None;
                                                    }
                                                    fileoff = segment.fileoff;
                                                    break;
                                                } else {
                                                    section -= segment.nsects as usize;
                                                }
                                            }
                                            let start = (fileoff + list.n_value) as usize;
                                            // MachO doesn't include symbol lengths, so we try to find the next symbol and if we can't, we just assume it takes up the remainder of the section.
                                            let previous_symbol = mach
                                                .symbols()
                                                .filter_map(|other| match other {
                                                    Ok((_, other)) => {
                                                        if other.n_sect == list.n_sect
                                                            && other.n_value > list.n_value
                                                        {
                                                            Some(other.n_value)
                                                        } else {
                                                            None
                                                        }
                                                    }
                                                    Err(_) => None,
                                                })
                                                .min()
                                                .map(|off| fileoff + off);
                                            match previous_symbol {
                                                Some(end) => Some((
                                                    Some(name.to_owned()),
                                                    start,
                                                    end as usize,
                                                )),
                                                None => {
                                                    eprintln!(
                                                        "Can't determine the size of {}.",
                                                        name
                                                    );
                                                    None
                                                }
                                            }
                                        }
                                    }
                                })
                                .chain(
                                    mach.exports()
                                        .map_err(GoblinYaxError::Goblin)?
                                        .into_iter()
                                        .map(|export| {
                                            (
                                                Some(export.name),
                                                export.offset as usize,
                                                (export.offset as usize + export.size),
                                            )
                                        }),
                                ),
                        )?
                    }
                    Object::Archive(ar) => {
                        let mut result = BTreeMap::new();
                        for (_, member, _) in ar.summarize().into_iter() {
                            result.extend(extract(
                                &buffer[member.offset as usize
                                    ..(member.offset as usize + member.size())],
                            )?);
                        }
                        result
                    }
                    Object::Unknown(_) => {
                        return Err(GoblinYaxError::Unrecognized);
                    }
                },
            )
        }

        Ok(GoblinYax {
            funcs: extract(&buffer)?,
        })
    }

    fn get(&self, name: &str) -> Option<&Self::Function> {
        self.funcs.get(name)
    }

    fn functions<'a>(&'a self) -> Box<dyn Iterator<Item = (&'a str, &'a Self::Function)> + 'a> {
        Box::new(self.funcs.iter().map(|(n, f)| (n.as_str(), f)))
    }
}
impl<A: yaxpeax_arch::Arch> crate::Function for GoblinYaxFunction<A>
where
    A::Instruction: YaxInstruction,
{
    type BasicBlock = GoblinYaxBlock<A>;

    fn blocks<'a>(&'a self) -> Box<dyn Iterator<Item = &'a Self::BasicBlock> + 'a> {
        Box::new(self.blocks.iter())
    }

    fn name<'a>(&'a self) -> std::borrow::Cow<'a, str> {
        std::borrow::Cow::Borrowed(&self.name)
    }
}
impl<A: yaxpeax_arch::Arch> crate::BasicBlock for GoblinYaxBlock<A>
where
    A::Instruction: YaxInstruction,
{
    type Instruction = A::Instruction;

    type Terminator = Option<A::Instruction>;

    fn get(&self, index: usize) -> &Self::Instruction {
        &self.instructions[index]
    }

    fn len(&self) -> usize {
        self.instructions.len()
    }

    fn name<'a>(&'a self) -> std::borrow::Cow<'a, str> {
        std::borrow::Cow::Owned(format!("{}", self.id))
    }

    fn terminator(&self) -> &Self::Terminator {
        &self.terminator
    }
}

fn demangle(symbol: &str) -> String {
    if let Ok(name) = rustc_demangle::try_demangle(symbol) {
        name.to_string()
    } else if let Ok(name) = cpp_demangle::Symbol::new(symbol) {
        name.to_string()
    } else {
        symbol.to_string()
    }
}
