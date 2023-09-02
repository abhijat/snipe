use std::collections::HashSet;
use std::ffi::OsStr;
use std::path::PathBuf;

use walkdir::WalkDir;

use crate::py_parser;
use crate::py_parser::ClassWithTests;

pub fn collect_python_test_files(root: &str) -> anyhow::Result<HashSet<PathBuf>> {
    let mut tests = HashSet::new();
    for entry in WalkDir::new(root) {
        let entry = entry?;
        let path = entry.path();
        if let Some(ext) = path.extension().and_then(OsStr::to_str) {
            if ext == "py" {
                tests.insert(path.to_owned());
            }
        }
    }
    Ok(tests)
}

pub fn collect_python_test_definitions(
    paths: &HashSet<PathBuf>,
) -> anyhow::Result<Vec<ClassWithTests>> {
    let mut tests = Vec::new();
    for path in paths {
        let test_classes = py_parser::find_tests_in_source(&path)?;
        tests.extend(test_classes);
    }

    Ok(tests)
}
