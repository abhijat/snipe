use std::fmt::{Display, Formatter};
use std::{
    fmt, fs,
    path::{Path, PathBuf},
};

use anyhow::Result;
use rustpython_parser::{
    ast::{ExprCall, StmtClassDef, StmtFunctionDef, Suite},
    Parse,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ClassWithTests {
    pub(crate) source_path: PathBuf,
    pub(crate) tests: Vec<String>,
    pub(crate) class_name: String,
}

impl Display for ClassWithTests {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}::{}",
            self.source_path.to_string_lossy(),
            self.class_name
        )
    }
}

pub fn find_tests_in_source(p: &Path) -> Result<Vec<ClassWithTests>> {
    let content = fs::read_to_string(p)?;
    let program = Suite::parse(&content, &p.to_string_lossy())?;
    let mut test_classes = Vec::new();
    for stmt in program {
        match stmt {
            rustpython_parser::ast::Stmt::ClassDef(class) => {
                let tests = collect_test_fns(&class);
                if !tests.is_empty() {
                    let class = ClassWithTests {
                        source_path: p.to_owned(),
                        tests,
                        class_name: class.name.to_string(),
                    };
                    test_classes.push(class);
                }
            }
            _ => {}
        }
    }
    Ok(test_classes)
}

fn collect_test_fns(class: &StmtClassDef) -> Vec<String> {
    let mut tests = Vec::new();
    for item in &class.body {
        match item {
            rustpython_parser::ast::Stmt::FunctionDef(f) => {
                if is_test_fn(&f) {
                    tests.push(f.name.to_string());
                }
            }
            _ => {}
        }
    }
    tests
}

fn is_test_fn(f: &StmtFunctionDef) -> bool {
    for d in &f.decorator_list {
        match d {
            rustpython_parser::ast::Expr::Call(call) => {
                if is_cluster_decorator(&call) {
                    return true;
                }
            }
            _ => {}
        }
    }

    false
}

fn is_cluster_decorator(call: &ExprCall) -> bool {
    match *call.func {
        rustpython_parser::ast::Expr::Name(ref name) => name.id.as_str() == "cluster",
        _ => false,
    }
}
