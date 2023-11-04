use std::collections::HashMap;
use std::fmt;
use std::fmt::{Display, Formatter};
use std::fs::{self};
use std::io::{stdin, stdout, Write};

use anyhow::{anyhow, Result};
use clap::{command, ArgGroup, Parser};

use cmake_parser::structures::RpTest;
use py_parser::ClassWithTests;
use scanners::{cmake, python};

use crate::config::{
    create_data_file_handle, get_data_file_handle, load_configuration, CommandEnv,
    CommandRunConfig, ScanConfig,
};
use crate::shell_commands::{run_cc_test, run_py_test};

mod cmake_parser;
pub mod config;
mod py_parser;
pub mod shell_commands;

mod scanners;

const CC_DB_FNAME: &'static str = "cc.json";
const PY_DB_FNAME: &'static str = "py.json";

fn select_from_list<T>(items: Vec<T>, name: &str) -> Result<Option<T>>
where
    T: Display,
{
    println!("Multiple matches found for {name}");
    loop {
        println!("Please select one of the following matching items (q to quit): ");
        for (index, item) in items.iter().enumerate() {
            println!("[{}] {}", index + 1, item);
        }
        let mut buf = String::new();

        print!(">> ");
        stdout().flush()?;

        stdin().read_line(&mut buf)?;

        let buf = buf.trim().to_lowercase();
        if buf == "q" {
            return Err(anyhow!("no test selected!"));
        }

        let choice: usize = buf.parse()?;
        if choice < 1 || choice > items.len() - 1 {
            println!("that is an invalid choice!");
        }

        return Ok(items.into_iter().skip(choice - 1).next());
    }
}

pub fn parse_env_file() -> Result<HashMap<String, String>> {
    let data = fs::read_to_string(".env")?;
    let mut map: HashMap<String, String> = Default::default();
    for row in data.lines() {
        let mut tokens = row.split("=").into_iter();
        map.insert(
            tokens.next().unwrap().trim().to_owned(),
            tokens.next().unwrap().trim().to_owned(),
        );
    }
    Ok(map)
}

pub enum TestKind {
    Cc,
    Py,
}

#[derive(Parser)]
#[command(author, version, about)]
#[clap(group(ArgGroup::new("test-kind").required(true).args(["cc", "py", "cli_content"])))]
pub struct Cli {
    #[clap(long, value_name = "C++ test name")]
    cc: Option<String>,

    #[clap(long, value_name = "Ducktape test name")]
    py: Option<String>,

    #[arg(short, long, help = "Edit command before running test")]
    edit: bool,

    #[arg(
        long,
        value_name = "Auto-complete",
        help = "Provide test names for auto completion"
    )]
    pub cli_content: Option<String>,
}

#[derive(Clone)]
pub enum TestSuite {
    C(RpTest),
    P(ClassWithTests),
    None,
}

impl Display for TestSuite {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            TestSuite::C(t) => write!(f, "{}", t),
            TestSuite::P(t) => write!(f, "{}", t),
            TestSuite::None => write!(f, "None"),
        }
    }
}

impl TestSuite {
    fn matches(&self, name: &str) -> bool {
        match &self {
            TestSuite::C(t) => t.tests.iter().any(|t| t == name),
            TestSuite::P(t) => t.tests.iter().any(|t| t == name),
            TestSuite::None => false,
        }
    }
}

pub struct SearchAndExecute {
    kind: TestKind,
    name: String,
    edit: bool,
    scan_config: ScanConfig,
    command_config: CommandRunConfig,
    command_environment: CommandEnv,
}

fn get_db_file(kind: &TestKind) -> &'static str {
    match kind {
        TestKind::Cc => CC_DB_FNAME,
        TestKind::Py => PY_DB_FNAME,
    }
}

impl SearchAndExecute {
    fn file_name(&self) -> &str {
        get_db_file(&self.kind)
    }

    fn scan_and_store_definitions(&self) -> Result<()> {
        let tests_json = match self.kind {
            TestKind::Cc => {
                let tests = cmake::collect_cmake_test_definitions(&self.scan_config.cc_test_root)?;
                serde_json::to_string_pretty(&tests)?
            }
            TestKind::Py => {
                let test_paths = python::collect_python_test_files(&self.scan_config.py_test_root)?;
                let tests = python::collect_python_test_definitions(&test_paths)?;
                serde_json::to_string_pretty(&tests)?
            }
        };

        let mut handle = create_data_file_handle(self.file_name())?;
        handle.write_all(tests_json.as_bytes())?;
        Ok(())
    }

    fn load_tests_from_db(&self) -> Result<Vec<TestSuite>> {
        let db = get_data_file_handle(self.file_name())?.expect("unexpected missing data file");
        let tests = match self.kind {
            TestKind::Cc => {
                let tests: Vec<RpTest> = serde_json::from_reader(db)?;
                tests.into_iter().map(|t| TestSuite::C(t)).collect()
            }
            TestKind::Py => {
                let tests: Vec<ClassWithTests> = serde_json::from_reader(db)?;
                tests.into_iter().map(|t| TestSuite::P(t)).collect()
            }
        };
        Ok(tests)
    }

    fn find_matching_tests(&self) -> Result<Vec<TestSuite>> {
        let tests = self.load_tests_from_db()?;
        let mut matching = Vec::new();
        for test in tests {
            if test.matches(&self.name) {
                matching.push(test.clone());
            }
        }
        Ok(matching)
    }

    pub fn ensure_db_exists(&self) -> Result<()> {
        let handle = get_data_file_handle(self.file_name())?;
        if handle.is_none() {
            self.scan_and_store_definitions()?;
        }

        Ok(())
    }

    pub fn find_test(&self) -> Result<TestSuite> {
        let mut matching = self.find_matching_tests()?;
        if matching.is_empty() {
            println!("test not found in cache, rescanning...");
            self.scan_and_store_definitions()?;
            matching = self.find_matching_tests()?;
            if matching.is_empty() {
                return Ok(TestSuite::None);
            }
        }

        if matching.len() == 1 {
            Ok(matching.into_iter().next().unwrap())
        } else {
            match select_from_list(matching, &self.name)? {
                None => Ok(TestSuite::None),
                Some(t) => Ok(t),
            }
        }
    }

    pub fn run_test(&self, f: TestSuite) -> Result<()> {
        match f {
            TestSuite::C(test) => run_cc_test(
                test,
                self.name.clone(),
                self.edit,
                &self.command_config,
                &self.command_environment.envs,
            ),
            TestSuite::P(test) => run_py_test(
                test,
                self.name.clone(),
                self.edit,
                &self.command_config,
                &self.command_environment.envs,
            ),
            TestSuite::None => {
                println!("no test found");
                Ok(())
            }
        }
    }

    fn do_autocomplete(command_line: &str) -> Result<Vec<String>> {
        let tokens: Vec<_> = command_line.split(",").collect();
        let kind = if tokens.iter().any(|token| *token == "cc") {
            Some(TestKind::Cc)
        } else if tokens.iter().any(|token| *token == "py") {
            Some(TestKind::Py)
        } else {
            return Err(anyhow!("no test kind"));
        };

        let scan_config = load_configuration(None).expect("Failed to load scan config");
        let command_config = load_configuration(None).expect("Failed to load command config");
        let command_environment = load_configuration(None).expect("Failed to load command envs");

        let sar = Self {
            kind: kind.unwrap(),
            name: "".to_owned(),
            edit: false,
            scan_config,
            command_config,
            command_environment,
        };

        sar.ensure_db_exists()?;
        let tests = sar.load_tests_from_db()?;
        let mut test_names = vec![];
        for test_suite in tests {
            match test_suite {
                TestSuite::C(rp_test) => test_names.extend(rp_test.tests),
                TestSuite::P(py) => test_names.extend(py.tests),
                TestSuite::None => {}
            }
        }

        Ok(test_names)
    }

    pub fn autocomplete(command_line: &str) {
        let tokens = Self::do_autocomplete(command_line).unwrap_or(Default::default());
        println!("{}", tokens.join(" "));
    }
}

impl From<Cli> for SearchAndExecute {
    fn from(value: Cli) -> Self {
        let (kind, name) = if let Some(cc) = value.cc {
            (TestKind::Cc, cc)
        } else if let Some(py) = value.py {
            (TestKind::Py, py)
        } else {
            panic!("unexpected run config")
        };
        let scan_config = load_configuration(None).expect("Failed to load scan config");
        let command_config = load_configuration(None).expect("Failed to load command config");
        let command_environment = load_configuration(None).expect("Failed to load command envs");
        Self {
            kind,
            name,
            edit: value.edit,
            scan_config,
            command_config,
            command_environment,
        }
    }
}
