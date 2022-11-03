pub mod goblin_yax;
pub mod llvm;
use std::{
    borrow::Cow,
    collections::{BTreeMap, BTreeSet},
    fmt::Display,
    path::Path,
};

/// Indicates what function(s) should be compared from the programs provided
pub enum FunctionName {
    /// Compare two functions of different names
    ///
    /// This is intended to be used when functions have been through name mangling that might make them structurally similar even though, say, template monomorphization even though the two names are different
    Different(String, String),
    /// Compare two functions of the same name
    Same(String),
    /// Compare any functions with identical names
    Unspecified,
}

/// A source program that can be used as input for diffing
///
/// Although this is called "program", it represent any collection of files and could be a complete program, a library, a partially compiled object file, methods of a single class, etc.
pub trait Program: Sized {
    /// The cost of inserting a gap into one side of instruction stream
    ///
    /// This is normally negative and a smaller value will trigger the diff to partition mismatching chunks of code; values closer to zero will favour aligning mismatching instructions
    const GAP: i32;

    /// The type of a function in the program
    type Function: Function;

    /// An error that can be produced if the file provided by the user is malformed
    type ParseError: Display;
    /// Options that are provided by the caller to control how parsing is performed
    type ParseOptions: Copy;
    /// Parsing an input file and produce a program for diffing
    fn parse(file: impl AsRef<Path>, options: Self::ParseOptions)
        -> Result<Self, Self::ParseError>;
    /// Retrive a function by name, if it exists
    fn get(&self, name: &str) -> Option<&Self::Function>;
    /// Iterate over all functions available
    fn functions<'a>(&'a self) -> Box<dyn Iterator<Item = &'a Self::Function> + 'a>;
}

/// A function for diffing
///
/// The diff works on the control flow graph level, so a function is a collection of basic blocks
pub trait Function {
    /// The type of a basic block in a function
    type BasicBlock: BasicBlock;
    /// The basic blocks associated with this function
    ///
    /// The diff algorithm may call this multiple times and the order of the basic blocks must be the same for each call
    fn blocks<'a>(&'a self) -> Box<dyn Iterator<Item = &'a Self::BasicBlock> + 'a>;
    /// The name of the function, as it should be displayed to the user
    fn name<'a>(&'a self) -> Cow<'a, str>;
}

/// A basic block in a function
///
/// Each basic block is a list of instructions that terminates in a flow control instruction (_e.g._, conditional branch). Functionally, terminal instructions and the body instructions can be different types, though they can be the same.
pub trait BasicBlock {
    /// The type of a body instruction
    type Instruction: Instruction;
    /// The type of the terminal (flow control) instruction
    type Terminator: Instruction;
    /// Get a body instruction by index
    ///
    /// This function can panic if the index is out-of-bounds
    fn get(&self, index: usize) -> &Self::Instruction;
    /// The number of body instructions in the block (_i.e._, excluding the terminal instruction)
    fn len(&self) -> usize;
    /// The name of the block
    ///
    /// This is arbitrary and if meaningful names are not available to display, using an incrementing number is acceptable.
    fn name<'a>(&'a self) -> Cow<'a, str>;
    /// The final flow control instructionl
    fn terminator(&self) -> &Self::Terminator;
}
/// A single instruction
///
/// This trait is used for both body and terminal instructions
pub trait Instruction {
    /// A score threshold where two instructions are considered an exact match. That is, they should not be highlighted as a diff
    const EQUIVALENT: i32;
    /// Compute the similarity score for two instructions
    ///
    /// This should be a positive integer if the instructions match or zero if they do not. It is possible to create a gradient of similarity. For instance, it might make sense to have an exact match be 4 and any two floating point math operations score 2 since they are more likely to be interchanged than a floating point operation and a load/store.
    fn score(&self, other: &Self) -> i32;
    /// Display the instruction for the user to consume
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

/// The output of a diff process
pub trait IntoDiffResult {
    /// The type of a single row comparing two instructions
    type Row;
    // Create a row that describes two basic blocks being compared given their names
    fn block_row(left: Cow<str>, right: Cow<str>) -> Self::Row;
    /// Create a row comparing two instructions and how they relate
    fn row(left: Cow<str>, right: Cow<str>, kind: MatchDirection) -> Self::Row;
    /// Create a complete function comparison given their names and the individual rows of their instructions
    fn function(left_name: Cow<str>, right_name: Cow<str>, rows: Vec<Self::Row>) -> Self;
}

/// The location where a function was discovered
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum FunctionLocation {
    /// In the left-hand file of a diff
    Left,
    /// In the right-hand file of a diff
    Right,
    /// In either/both file(s) of a diff
    Both,
}

impl FunctionLocation {
    /// The name of a location
    pub fn name(&self) -> &'static str {
        match self {
            FunctionLocation::Left => "left-hand",
            FunctionLocation::Right => "right-hand",
            FunctionLocation::Both => "either",
        }
    }
}

/// The type of match at this location
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum MatchDirection {
    /// The instruction from left-hand file is absent and the right-hand is supplied
    GapLeft,
    /// The instruction from right-hand file is absent and the left-hand is supplied
    GapRight,
    /// Instructions from both files are present; the included Boolean is true if the match is exact
    Align(bool),
}

/// An error produced by the diff process
pub enum Error<P: Program> {
    /// Functions could not be located from the input programs
    NoMatch(FunctionLocation),
    /// The input file could not be parsed
    ParseError(FunctionLocation, P::ParseError),
}

/// Parse and compare two programs
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
            let right: BTreeMap<_, _> = right.functions().map(|func| (func.name(), func)).collect();
            let result: Vec<_> = left
                .functions()
                .filter_map(|left_fn| {
                    right
                        .get(left_fn.name().as_ref())
                        .map(|&right_fn| (left_fn, right_fn))
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
