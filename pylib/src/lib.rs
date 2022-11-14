use ctflgrdifflib::{goblin_yax::GoblinYax, *};
use pyo3::{
    exceptions::{PyIndexError, PyValueError},
    prelude::*,
    types::IntoPyDict,
};
use yaxpeax_arch::StandardDecodeError;
use yaxpeax_arm::{
    armv7,
    armv8::a64::{self},
};
use yaxpeax_x86::long_mode;
use yaxpeax_x86::protected_mode;

trait ToPyError {
    fn to_py_err_str(self) -> String;
}

impl ToPyError for String {
    fn to_py_err_str(self) -> String {
        self
    }
}
impl ToPyError for a64::DecodeError {
    fn to_py_err_str(self) -> String {
        self.to_string()
    }
}
impl ToPyError for armv7::DecodeError {
    fn to_py_err_str(self) -> String {
        self.to_string()
    }
}
impl ToPyError for protected_mode::DecodeError {
    fn to_py_err_str(self) -> String {
        self.to_string()
    }
}
impl ToPyError for long_mode::DecodeError {
    fn to_py_err_str(self) -> String {
        self.to_string()
    }
}
impl ToPyError for StandardDecodeError {
    fn to_py_err_str(self) -> String {
        self.to_string()
    }
}
impl<A: yaxpeax_arch::Arch> ToPyError for goblin_yax::GoblinYaxError<A>
where
    A::DecodeError: ToPyError,
{
    fn to_py_err_str(self) -> String {
        format!("{}", self)
    }
}
fn convert_error<P: Program>(py: Python, e: ctflgrdifflib::Error<P>) -> PyErr
where
    P::ParseError: ToPyError,
{
    match e {
        Error::NoMatch(l) => PyErr::from_value(PyIndexError::new_err(l.name()).value(py)),
        Error::ParseError(l, e) => {
            let mut e = e.to_py_err_str();
            e.push_str(" (");
            e.push_str(l.name());
            e.push_str(")");
            PyErr::from_value(PyValueError::new_err(e).value(py))
        }
    }
}

struct PyDiff(String, String, Vec<Row>);
type Row = (Option<&'static str>, String, String);

impl IntoDiffResult for PyDiff {
    type Row = Row;

    fn block_row(left: std::borrow::Cow<str>, right: std::borrow::Cow<str>) -> Self::Row {
        (None, left.to_string(), right.to_string())
    }

    fn row(
        left: std::borrow::Cow<str>,
        right: std::borrow::Cow<str>,
        kind: MatchDirection,
    ) -> Self::Row {
        (
            Some(match kind {
                MatchDirection::GapLeft => "left",
                MatchDirection::GapRight => "right",
                MatchDirection::Align(exact) => {
                    if exact {
                        "match"
                    } else {
                        "mismatch"
                    }
                }
            }),
            left.to_string(),
            right.to_string(),
        )
    }

    fn function(
        left_name: std::borrow::Cow<str>,
        right_name: std::borrow::Cow<str>,
        rows: Vec<Self::Row>,
    ) -> Self {
        PyDiff(left_name.to_string(), right_name.to_string(), rows)
    }
}

#[pyfunction]
fn make_diff(
    format: &str,
    left_file: &str,
    right_file: &str,
    left_name: Option<String>,
    right_name: Option<String>,
) -> PyResult<PyObject> {
    Python::with_gil(|py| {
        let function_name = match (left_name, right_name) {
            (None, None) => FunctionName::Unspecified,
            (None, Some(_)) => {
                return Err(PyErr::from_value(
                    PyValueError::new_err("Right-hand function provided, but left is missing")
                        .value(py),
                ));
            }
            (Some(v), None) => FunctionName::Same(v),
            (Some(l), Some(r)) => FunctionName::Different(l, r),
        };
        let (has_diff, diffs): (_, Vec<PyDiff>) = match format {
            "ll" | "ll-ir" | "llir" => {
                compute_diff::<llvm_ir::Module, _>(left_file, right_file, function_name, true)
                    .map_err(|e| convert_error(py, e))
            }
            "ll-bc" | "llbc" => {
                compute_diff::<llvm_ir::Module, _>(left_file, right_file, function_name, false)
                    .map_err(|e| convert_error(py, e))
            }
            "arm64" | "aarch64" | "armv8" => compute_diff::<
                GoblinYax<yaxpeax_arm::armv8::a64::ARMv8>,
                _,
            >(left_file, right_file, function_name, ())
            .map_err(|e| convert_error(py, e)),
            "arm32" | "aarch32" | "armv7" => compute_diff::<
                GoblinYax<yaxpeax_arm::armv8::a64::ARMv8>,
                _,
            >(left_file, right_file, function_name, ())
            .map_err(|e| convert_error(py, e)),
            "avr" => compute_diff::<GoblinYax<yaxpeax_avr::AVR>, _>(
                left_file,
                right_file,
                function_name,
                (),
            )
            .map_err(|e| convert_error(py, e)),
            "x86" | "x86-32" | "x86_32" | "i386" | "i686" => {
                compute_diff::<GoblinYax<yaxpeax_x86::x86_32>, _>(
                    left_file,
                    right_file,
                    function_name,
                    (),
                )
                .map_err(|e| convert_error(py, e))
            }
            "x64" | "x86-64" | "x86_64" => compute_diff::<GoblinYax<yaxpeax_x86::x86_64>, _>(
                left_file,
                right_file,
                function_name,
                (),
            )
            .map_err(|e| convert_error(py, e)),
            fmt => Err(PyErr::from_value(
                PyValueError::new_err(format!("Unsupported format {}", fmt)).value(py),
            )),
        }?;

        Ok((
            has_diff,
            diffs
                .into_iter()
                .map(|PyDiff(l, r, rows)| ((l, r).to_object(py), rows.to_object(py)))
                .into_py_dict(py),
        )
            .to_object(py))
    })
}

#[pymodule]
fn pyctflgrdiff(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(make_diff, m)?)?;
    Ok(())
}
