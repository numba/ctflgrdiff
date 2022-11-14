use ctflgrdifflib::*;
use pyo3::{
    exceptions::{PyIndexError, PyValueError},
    prelude::*,
    types::IntoPyDict,
};

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
        let (has_diff, diffs) =
            compute_diff_with_format::<PyDiff>(format, left_file, right_file, function_name)
                .map_err(|e| match e {
                    FormatError::BadFormat => PyErr::from_value(
                        PyValueError::new_err(format!("Unknown assembly format {}", format))
                            .value(py),
                    ),
                    FormatError::NoMatch(l) => {
                        PyErr::from_value(PyIndexError::new_err(l.name()).value(py))
                    }
                    FormatError::ParseError(l, mut e) => {
                        e.push_str(" (");
                        e.push_str(l.name());
                        e.push_str(")");
                        PyErr::from_value(PyValueError::new_err(e).value(py))
                    }
                })?;

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
