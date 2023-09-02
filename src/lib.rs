use std::collections::HashMap;
use std::fmt;
use std::fmt::{Display, Formatter};
use std::fs::{self};
use std::io::{stdin, stdout, Write};

use anyhow::{anyhow, Result};
use clap::{ArgGroup, Parser};

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
#[clap(group(ArgGroup::new("test-kind").required(true).args(["cc", "py"])))]
pub struct Cli {
    #[clap(long, value_name = "C++ test name")]
    cc: Option<String>,

    #[clap(long, value_name = "Ducktape test name")]
    py: Option<String>,

    #[arg(short, long, help = "Edit command before running test")]
    edit: bool,

    #[arg(
        short,
        long,
        value_name = "Config file",
        help = "Seed defaults from config file (and copy to default location for future runs)"
    )]
    config_file: Option<String>,
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

impl SearchAndExecute {
    fn file_name(&self) -> &str {
        match self.kind {
            TestKind::Cc => "cc.json",
            TestKind::Py => "py.json",
        }
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
        let scan_config =
            load_configuration(value.config_file).expect("Failed to load scan config");
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
