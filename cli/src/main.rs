use std::{borrow::Cow, path::Path};

use clap::Parser;
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ctflgrdifflib::{
    compute_diff, goblin_yax::GoblinYax, FunctionName, IntoDiffResult, MatchDirection, Program,
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
            show_diff::<llvm_ir::Module>(args.left_file, args.right_file, function_name, false)
        }
        "arm64" | "aarch64" | "armv8" => show_diff::<GoblinYax<yaxpeax_arm::armv8::a64::ARMv8>>(
            args.left_file,
            args.right_file,
            function_name,
            (),
        ),
        "arm32" | "aarch32" | "armv7" => show_diff::<GoblinYax<yaxpeax_arm::armv8::a64::ARMv8>>(
            args.left_file,
            args.right_file,
            function_name,
            (),
        ),
        "avr" => show_diff::<GoblinYax<yaxpeax_avr::AVR>>(
            args.left_file,
            args.right_file,
            function_name,
            (),
        ),
        "x86" | "x86-32" | "x86_32" | "i386" | "i686" => {
            show_diff::<GoblinYax<yaxpeax_x86::x86_32>>(
                args.left_file,
                args.right_file,
                function_name,
                (),
            )
        }
        "x64" | "x86-64" | "x86_64" => show_diff::<GoblinYax<yaxpeax_x86::x86_64>>(
            args.left_file,
            args.right_file,
            function_name,
            (),
        ),
        fmt => {
            eprintln!("Can't parse “{}” files. Sorry.", fmt);
            2
        }
    });
}

struct ConsoleOutput(String, Table<'static>);
impl IntoDiffResult for ConsoleOutput {
    type Row = Row<'static>;

    fn block_row(left: Cow<str>, right: Cow<str>) -> Self::Row {
        Row::new([left.to_string(), right.to_string()].into_iter())
            .style(Style::default().fg(Color::LightBlue))
    }

    fn row(left: Cow<str>, right: Cow<str>, kind: MatchDirection) -> Self::Row {
        Row::new([left.to_string(), right.to_string()].into_iter()).style(Style::default().bg(
            match kind {
                MatchDirection::Align(true) => Color::Black,
                MatchDirection::Align(false) => Color::Blue,
                MatchDirection::GapLeft => Color::Cyan,
                MatchDirection::GapRight => Color::Magenta,
            },
        ))
    }

    fn function(left_name: Cow<str>, right_name: Cow<str>, rows: Vec<Self::Row>) -> Self {
        ConsoleOutput(
            if left_name.as_ref() == right_name.as_ref() {
                left_name.to_string()
            } else {
                format!("{} vs {}", left_name, right_name)
            },
            Table::new(rows)
                .header(
                    Row::new([left_name.to_string(), right_name.to_string()].into_iter())
                        .style(Style::default().fg(Color::White)),
                )
                .widths(&[Constraint::Percentage(50), Constraint::Percentage(50)])
                .style(Style::default().fg(Color::White))
                .highlight_symbol(">>"),
        )
    }
}

fn show_diff<P: Program>(
    left: impl AsRef<Path>,
    right: impl AsRef<Path>,
    name: FunctionName,
    options: P::ParseOptions,
) -> i32 {
    match compute_diff::<P, ConsoleOutput>(left, right, name, options) {
        Err(e) => match e {
            ctflgrdifflib::Error::NoMatch(location) => {
                eprintln!("Cannot find function in {} file", location.name());
                3
            }
            ctflgrdifflib::Error::ParseError(location, e) => {
                eprintln!("Failed to parse {} file: {}", location.name(), e);
                3
            }
        },
        Ok((has_diff, diffs)) => {
            if diffs.is_empty() {
                return 0;
            } else {
                let titles = Tabs::new(
                    diffs
                        .iter()
                        .map(|output| Spans::from(vec![Span::raw(&output.0)]))
                        .collect(),
                )
                .block(Block::default().title("Function").borders(Borders::ALL));
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
                            rect.render_stateful_widget(
                                diffs[active_tab].1.clone(),
                                chunks[1],
                                &mut table_state,
                            );
                        })
                        .unwrap();

                    if let Event::Key(key) =
                        crossterm::event::read().expect("Failed to read from terminal")
                    {
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
        }
    }
}
