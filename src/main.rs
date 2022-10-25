mod goblin_yax;
mod llvm;
use std::{
    borrow::Cow,
    collections::{BTreeMap, BTreeSet},
    fmt::Display,
    path::Path,
};

use clap::Parser;
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use tui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    text::{Span, Spans},
    widgets::{Block, Borders, Row, Table, TableState, Tabs},
    Terminal,
};
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// The function name to compare
    #[arg(short, long)]
    name: Option<String>,
    /// If the function is different named in the the two files (e.g., when name mangling includes type information), this is the name of the function in the right-hand file
    #[arg(long)]
    right_name: Option<String>,

    /// The file format to parse
    #[arg(short, long)]
    format: String,
    left_file: String,
    right_file: String,
}
enum FunctionName {
    Different(String, String),
    Same(String),
    Unspecified,
}
trait Program: Sized {
    const GAP: i32;
    type ParseError: Display;
    type Function: Function;
    fn parse(file: impl AsRef<Path>) -> Result<Self, Self::ParseError>;
    fn get(&self, name: &str) -> Option<&Self::Function>;
    fn functions<'a>(&'a self) -> Box<dyn Iterator<Item = (&'a str, &'a Self::Function)> + 'a>;
}
trait Function {
    type BasicBlock: BasicBlock;
    fn blocks<'a>(&'a self) -> Box<dyn Iterator<Item = &'a Self::BasicBlock> + 'a>;
    fn name<'a>(&'a self) -> Cow<'a, str>;
}
trait BasicBlock {
    type Instruction: Instruction;
    type Terminator: Instruction;
    fn get(&self, index: usize) -> &Self::Instruction;
    fn len(&self) -> usize;
    fn name<'a>(&'a self) -> Cow<'a, str>;
    fn terminator(&self) -> &Self::Terminator;
}
trait Instruction {
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
fn main() {
    let args = Args::parse();
    let function_name = match (args.name, args.right_name) {
        (Some(left), Some(right)) => FunctionName::Different(left, right),
        (Some(left), None) => FunctionName::Same(left),
        (None, None) => FunctionName::Unspecified,
        (None, Some(_)) => {
            eprintln!(
                "Only right-hand function name is supplied. Don't know what to do with that."
            );
            std::process::exit(4);
        }
    };

    std::process::exit(match args.format.as_str() {
        "ll-bc" | "llbc" => {
            show_diff::<llvm_ir::Module>(args.left_file, args.right_file, function_name)
        }
        "arm64" | "aarch64" | "armv8" => show_diff::<
            goblin_yax::GoblinYax<yaxpeax_arm::armv8::a64::ARMv8>,
        >(args.left_file, args.right_file, function_name),
        "arm32" | "aarch32" | "armv7" => show_diff::<
            goblin_yax::GoblinYax<yaxpeax_arm::armv8::a64::ARMv8>,
        >(args.left_file, args.right_file, function_name),
        "avr" => show_diff::<goblin_yax::GoblinYax<yaxpeax_avr::AVR>>(
            args.left_file,
            args.right_file,
            function_name,
        ),
        "x86" | "x86-32" | "x86_32" | "i386" | "i686" => {
            show_diff::<goblin_yax::GoblinYax<yaxpeax_x86::x86_32>>(
                args.left_file,
                args.right_file,
                function_name,
            )
        }
        "x64" | "x86-64" | "x86_64" => show_diff::<goblin_yax::GoblinYax<yaxpeax_x86::x86_64>>(
            args.left_file,
            args.right_file,
            function_name,
        ),
        fmt => {
            eprintln!("Can't parse “{}” files. Sorry.", fmt);
            2
        }
    });
}

fn show_diff<P: Program>(
    left: impl AsRef<Path>,
    right: impl AsRef<Path>,
    name: FunctionName,
) -> i32 {
    fn find_functions<'a, T>(
        left: Option<&'a T>,
        right: Option<&'a T>,
    ) -> Result<Box<dyn Iterator<Item = (&'a T, &'a T)> + 'a>, i32> {
        match (left, right) {
            (Some(left), Some(right)) => Ok(Box::new(std::iter::once((left, right)))),
            (None, Some(_)) => {
                eprintln!("Cannot find function in left-hand file");
                Err(3)
            }
            (Some(_), None) => {
                eprintln!("Cannot find function in right-hand file");
                Err(3)
            }

            (None, None) => {
                eprintln!("Cannot find functions in either file");
                Err(3)
            }
        }
    }
    let left = match P::parse(left) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Failed to parse left-hand file: {}", e);
            return 3;
        }
    };
    let right = match P::parse(right) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Failed to parse right-hand file: {}", e);
            return 3;
        }
    };
    let pairs = match name {
        FunctionName::Different(left_name, right_name) => {
            match find_functions(left.get(left_name.as_str()), right.get(right_name.as_str())) {
                Ok(i) => i,
                Err(rc) => {
                    return rc;
                }
            }
        }
        FunctionName::Same(name) => {
            match find_functions(left.get(name.as_str()), right.get(name.as_str())) {
                Ok(i) => i,
                Err(rc) => {
                    return rc;
                }
            }
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
                eprintln!("No functions in common.");
                return 3;
            }
            Box::new(result.into_iter())
        }
    };
    let mut has_diff = true;
    let mut titles = Vec::new();
    let mut diffs = Vec::new();
    for (left_func, right_func) in pairs {
        #[derive(Copy, Clone, Debug)]
        enum MatchDirection {
            GapLeft,
            GapRight,
            Align(bool),
        }
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
                table.push(
                    Row::new(vec![left_block.name(), right_block.name()])
                        .style(Style::default().fg(Color::LightBlue)),
                );
                let mut i = 0;
                let mut j = 0;
                for direction in path {
                    let (left, right, color) = match direction {
                        MatchDirection::Align(equivalent) => {
                            let result = (
                                left_block.get(i).render(),
                                right_block.get(j).render(),
                                if equivalent {
                                    Color::Black
                                } else {
                                    Color::Blue
                                },
                            );
                            if !equivalent {
                                has_diff = true;
                            }
                            i += 1;
                            j += 1;
                            result
                        }
                        MatchDirection::GapLeft => {
                            let result =
                                (Cow::Borrowed(""), right_block.get(j).render(), Color::Cyan);
                            j += 1;
                            has_diff = true;
                            result
                        }
                        MatchDirection::GapRight => {
                            let result = (
                                left_block.get(i).render(),
                                Cow::Borrowed(""),
                                Color::Magenta,
                            );
                            has_diff = true;
                            i += 1;
                            result
                        }
                    };
                    table.push(Row::new(vec![left, right]).style(Style::default().bg(color)));
                }
                table.push(
                    Row::new(vec![
                        left_block.terminator().render(),
                        right_block.terminator().render(),
                    ])
                    .style(Style::default().bg(if terminator_equivalent {
                        Color::Black
                    } else {
                        Color::Blue
                    })),
                );
            } else {
                has_diff = true;
                table.push(
                    Row::new(vec![left_block.name(), Cow::Borrowed("")])
                        .style(Style::default().fg(Color::LightBlue)),
                );
                for instruction in 0..left_block.len() {
                    table.push(
                        Row::new(vec![
                            left_block.get(instruction).render(),
                            Cow::Borrowed(""),
                        ])
                        .style(Style::default().bg(Color::Magenta)),
                    );
                }
                table.push(
                    Row::new(vec![left_block.terminator().render(), Cow::Borrowed("")])
                        .style(Style::default().bg(Color::Magenta)),
                );
            }
        }
        for (_, unused_block) in right_func
            .blocks()
            .enumerate()
            .filter(|(id, _)| !used_right_blocks.contains(id))
        {
            has_diff = true;
            table.push(
                Row::new(vec![Cow::Borrowed(""), unused_block.name()])
                    .style(Style::default().fg(Color::LightBlue)),
            );
            for instruction in 0..unused_block.len() {
                table.push(
                    Row::new(vec![
                        Cow::Borrowed(""),
                        unused_block.get(instruction).render(),
                    ])
                    .style(Style::default().bg(Color::Cyan)),
                );
            }
            table.push(
                Row::new(vec![Cow::Borrowed(""), unused_block.terminator().render()])
                    .style(Style::default().bg(Color::Cyan)),
            );
        }
        diffs.push(
            Table::new(table)
                .header(
                    Row::new(vec![left_func.name(), right_func.name()])
                        .style(Style::default().fg(Color::White)),
                )
                .widths(&[Constraint::Percentage(50), Constraint::Percentage(50)])
                .style(Style::default().fg(Color::White))
                .highlight_symbol(">>"),
        );
        titles.push(Spans::from(vec![Span::raw(
            if left_func.name().as_ref() == right_func.name().as_ref() {
                left_func.name().to_string()
            } else {
                format!("{} vs {}", left_func.name(), right_func.name())
            },
        )]));
    }
    if diffs.is_empty() {
        return 0;
    }
    let titles = Tabs::new(titles).block(Block::default().title("Function").borders(Borders::ALL));
    let mut table_state = TableState::default();
    let mut active_tab = 0;
    let mut stdout = std::io::stdout();
    let mut terminal = match enable_raw_mode()
        .and_then(|_| execute!(stdout, EnterAlternateScreen, EnableMouseCapture))
        .and_then(|_| Terminal::new(CrosstermBackend::new(stdout)))
    {
        Ok(t) => t,
        Err(e) => {
            eprintln!("Failed to initialize terminal: {}", e);
            return 100;
        }
    };
    loop {
        terminal
            .draw(|rect| {
                let size = rect.size();
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .margin(2)
                    .constraints([Constraint::Length(3), Constraint::Min(20)].as_ref())
                    .split(size);
                rect.render_widget(titles.clone().select(active_tab), chunks[0]);
                rect.render_stateful_widget(diffs[active_tab].clone(), chunks[1], &mut table_state);
            })
            .unwrap();

        if let Event::Key(key) = crossterm::event::read().expect("Failed to read from terminal") {
            match key.code {
                KeyCode::Esc | KeyCode::Char('x') | KeyCode::Char('q') => break,
                KeyCode::Right => {
                    if active_tab < diffs.len() - 1 {
                        active_tab += 1;
                        table_state.select(None);
                    }
                }
                KeyCode::Left => {
                    if active_tab > 0 {
                        active_tab -= 1;
                        table_state.select(None);
                    }
                }
                _ => {}
            }
        }
    }
    if let Err(e) = disable_raw_mode()
        .and_then(|_| {
            execute!(
                terminal.backend_mut(),
                LeaveAlternateScreen,
                DisableMouseCapture
            )
        })
        .and_then(|_| terminal.show_cursor())
    {
        eprintln!("Failed to reset terminal: {}", e);
    }
    if has_diff {
        1
    } else {
        0
    }
}
