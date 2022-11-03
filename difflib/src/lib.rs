pub mod goblin_yax;
pub mod llvm;
use std::{
    borrow::Cow,
    collections::{BTreeMap, BTreeSet},
    fmt::Display,
    path::Path,
};

pub enum FunctionName {
    Different(String, String),
    Same(String),
    Unspecified,
}
pub trait Program: Sized {
    const GAP: i32;
    type Function: Function;
    type ParseError: Display;
    type ParseOptions: Copy;
    fn parse(file: impl AsRef<Path>, options: Self::ParseOptions)
        -> Result<Self, Self::ParseError>;
    fn get(&self, name: &str) -> Option<&Self::Function>;
    fn functions<'a>(&'a self) -> Box<dyn Iterator<Item = (&'a str, &'a Self::Function)> + 'a>;
}
pub trait Function {
    type BasicBlock: BasicBlock;
    fn blocks<'a>(&'a self) -> Box<dyn Iterator<Item = &'a Self::BasicBlock> + 'a>;
    fn name<'a>(&'a self) -> Cow<'a, str>;
}
pub trait BasicBlock {
    type Instruction: Instruction;
    type Terminator: Instruction;
    fn get(&self, index: usize) -> &Self::Instruction;
    fn len(&self) -> usize;
    fn name<'a>(&'a self) -> Cow<'a, str>;
    fn terminator(&self) -> &Self::Terminator;
}
pub trait Instruction {
    const EQUIVALENT: i32;
    fn score(&self, other: &Self) -> i32;
    fn render<'a>(&self) -> Cow<'a, str>;
}
impl Instruction for () {
    const EQUIVALENT: i32 = 0;
    fn score(&self, _other: &Self) -> i32 {
        0
    }

    fn render<'a>(&self) -> Cow<'a, str> {
        Cow::Borrowed("")
    }
}
impl<T: Instruction> Instruction for Option<T> {
    const EQUIVALENT: i32 = T::EQUIVALENT;

    fn score(&self, other: &Self) -> i32 {
        match (self, other) {
            (Some(left), Some(right)) => left.score(right),
            (None, None) => T::EQUIVALENT,
            _ => 0,
        }
    }

    fn render<'a>(&self) -> Cow<'a, str> {
        match self {
            Some(inst) => inst.render(),
            None => Cow::Borrowed("<no instruction>"),
        }
    }
}

pub trait IntoDiffResult {
    type Row;
    fn block_row(left: Cow<str>, right: Cow<str>) -> Self::Row;
    fn row(left: Cow<str>, right: Cow<str>, kind: MatchDirection) -> Self::Row;
    fn function(left_name: Cow<str>, right_name: Cow<str>, rows: Vec<Self::Row>) -> Self;
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum FunctionLocation {
    Left,
    Right,
    Both,
}

impl FunctionLocation {
    pub fn name(&self) -> &'static str {
        match self {
            FunctionLocation::Left => "left-hand",
            FunctionLocation::Right => "right-hand",
            FunctionLocation::Both => "either",
        }
    }
}
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum MatchDirection {
    GapLeft,
    GapRight,
    Align(bool),
}

pub enum Error<P: Program> {
    NoMatch(FunctionLocation),
    ParseError(FunctionLocation, P::ParseError),
}

pub fn compute_diff<P: Program, D: IntoDiffResult>(
    left: impl AsRef<Path>,
    right: impl AsRef<Path>,
    name: FunctionName,
    options: P::ParseOptions,
) -> Result<(bool, Vec<D>), Error<P>> {
    fn find_functions<'a, T, P: Program>(
        left: Option<&'a T>,
        right: Option<&'a T>,
    ) -> Result<Box<dyn Iterator<Item = (&'a T, &'a T)> + 'a>, Error<P>> {
        match (left, right) {
            (Some(left), Some(right)) => Ok(Box::new(std::iter::once((left, right)))),
            (None, Some(_)) => Err(Error::NoMatch(FunctionLocation::Left)),
            (Some(_), None) => Err(Error::NoMatch(FunctionLocation::Right)),

            (None, None) => Err(Error::NoMatch(FunctionLocation::Both)),
        }
    }
    let left = P::parse(left, options).map_err(|e| Error::ParseError(FunctionLocation::Left, e))?;
    let right =
        P::parse(right, options).map_err(|e| Error::ParseError(FunctionLocation::Right, e))?;
    let pairs = match name {
        FunctionName::Different(left_name, right_name) => {
            find_functions(left.get(left_name.as_str()), right.get(right_name.as_str()))?
        }
        FunctionName::Same(name) => {
            find_functions(left.get(name.as_str()), right.get(name.as_str()))?
        }
        FunctionName::Unspecified => {
            let right: BTreeMap<_, _> = right.functions().collect();
            let result: Vec<_> = left
                .functions()
                .filter_map(|(fn_name, left_fn)| {
                    right.get(fn_name).map(|&right_fn| (left_fn, right_fn))
                })
                .collect();
            if result.is_empty() {
                return Err(Error::NoMatch(FunctionLocation::Both));
            }
            Box::new(result.into_iter())
        }
    };
    let mut has_diff = true;
    let mut diffs = Vec::new();
    for (left_func, right_func) in pairs {
        let mut table = Vec::new();

        let mut used_right_blocks = BTreeSet::new();
        for left_block in left_func.blocks() {
            let mut best_block = None;
            for (right_id, right_block) in right_func.blocks().enumerate() {
                let mut grid =
                    vec![vec![(0i32, None); right_block.len() + 1]; left_block.len() + 1];
                for i in 1..=left_block.len() {
                    grid[i][0] = (i as i32 * -P::GAP, Some(MatchDirection::GapRight));
                }
                for i in 1..=right_block.len() {
                    grid[0][i] = (i as i32 * -P::GAP, Some(MatchDirection::GapLeft));
                }
                for i in 0..left_block.len() {
                    for j in 0..right_block.len() {
                        let scores = [
                            {
                                let score = left_block.get(i).score(right_block.get(j));
                                (
                                    grid[i][j].0 + score,
                                    Some(MatchDirection::Align(
                                        score >= <<<P as Program>::Function as Function>::BasicBlock as BasicBlock>::Instruction::EQUIVALENT,
                                    )),
                                )
                            },
                            (grid[i + 1][j].0 - P::GAP, Some(MatchDirection::GapLeft)),
                            (grid[i][j + 1].0 - P::GAP, Some(MatchDirection::GapRight)),
                        ];
                        grid[i + 1][j + 1] =
                            scores.into_iter().max_by_key(|(score, _)| *score).unwrap();
                    }
                }
                let (mut score, mut direction) = grid[left_block.len()][right_block.len()];
                let terminator_score = left_block.terminator().score(right_block.terminator());
                score += terminator_score;

                if score > 0
                    && best_block
                        .as_ref()
                        .map(|(best_score, _, _, _, _)| score > *best_score)
                        .unwrap_or(true)
                {
                    let mut path = Vec::new();
                    let mut i = left_block.len();
                    let mut j = right_block.len();
                    loop {
                        if let Some(direction) = direction {
                            path.push(direction);
                        }
                        match direction {
                            None => {
                                break;
                            }
                            Some(MatchDirection::GapLeft) => {
                                j -= 1;
                            }
                            Some(MatchDirection::GapRight) => {
                                i -= 1;
                            }
                            Some(MatchDirection::Align(_)) => {
                                i -= 1;
                                j -= 1;
                            }
                        }
                        direction = grid[i][j].1;
                    }

                    path.reverse();
                    best_block = Some((
                        score,
                        terminator_score >= <<<P as Program>::Function as Function>::BasicBlock as BasicBlock>::Terminator::EQUIVALENT,
                        right_id,
                        right_block,
                        path,
                    ));
                }
            }
            if let Some((_, terminator_equivalent, right_id, right_block, path)) = best_block {
                used_right_blocks.insert(right_id);
                table.push(D::block_row(left_block.name(), right_block.name()));
                let mut i = 0;
                let mut j = 0;
                for direction in path {
                    let (left, right) = match direction {
                        MatchDirection::Align(equivalent) => {
                            let result = (left_block.get(i).render(), right_block.get(j).render());
                            if !equivalent {
                                has_diff = true;
                            }
                            i += 1;
                            j += 1;
                            result
                        }
                        MatchDirection::GapLeft => {
                            let result = (Cow::Borrowed(""), right_block.get(j).render());
                            j += 1;
                            has_diff = true;
                            result
                        }
                        MatchDirection::GapRight => {
                            let result = (left_block.get(i).render(), Cow::Borrowed(""));
                            has_diff = true;
                            i += 1;
                            result
                        }
                    };
                    table.push(D::row(left, right, direction));
                }
                table.push(D::row(
                    left_block.terminator().render(),
                    right_block.terminator().render(),
                    MatchDirection::Align(terminator_equivalent),
                ));
            } else {
                has_diff = true;
                table.push(D::block_row(left_block.name(), Cow::Borrowed("")));
                for instruction in 0..left_block.len() {
                    table.push(D::row(
                        left_block.get(instruction).render(),
                        Cow::Borrowed(""),
                        MatchDirection::GapRight,
                    ));
                }
                table.push(D::row(
                    left_block.terminator().render(),
                    Cow::Borrowed(""),
                    MatchDirection::Align(false),
                ));
            }
        }
        for (_, unused_block) in right_func
            .blocks()
            .enumerate()
            .filter(|(id, _)| !used_right_blocks.contains(id))
        {
            has_diff = true;
            table.push(D::block_row(Cow::Borrowed(""), unused_block.name()));
            for instruction in 0..unused_block.len() {
                table.push(D::row(
                    Cow::Borrowed(""),
                    unused_block.get(instruction).render(),
                    MatchDirection::GapLeft,
                ));
            }
            table.push(D::row(
                Cow::Borrowed(""),
                unused_block.terminator().render(),
                MatchDirection::GapLeft,
            ));
        }
        diffs.push(D::function(left_func.name(), right_func.name(), table));
    }
    Ok((has_diff, diffs))
}
