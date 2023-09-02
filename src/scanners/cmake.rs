use std::collections::HashSet;
use std::fs;
use std::path::Path;

use anyhow::anyhow;
use walkdir::WalkDir;

use crate::cmake_parser::parse_tests_from_file;
use crate::cmake_parser::structures::RpTest;

pub enum SplitOn {
    Delim(&'static str),
}

pub struct CcTest {
    tag: String,
    name: String,
}

impl CcTest {
    pub fn new(tag: &str, name: &str) -> Self {
        Self {
            tag: tag.to_owned(),
            name: name.to_owned(),
        }
    }
}

pub fn parse_test_name_from_source(
    data: &str,
    tags: &HashSet<String>,
    split_args_on: SplitOn,
) -> anyhow::Result<Vec<CcTest>> {
    let mut arg_groups = Vec::new();
    let mut lines = data.lines();
    while let Some(line) = lines.next() {
        let line = line.trim();
        for tag in tags {
            if line.starts_with(tag) {
                let mut buf = String::new();
                buf.push_str(line);
                while !buf.contains(')') {
                    buf.push_str(lines.next().ok_or(anyhow!("missing closing paren"))?);
                }
                let args: String = buf.split('(').skip(1).collect();
                let args = args.split(')').next().unwrap_or("");
                let args: Vec<_> = match split_args_on {
                    SplitOn::Delim(delim) => args
                        .split(delim)
                        .map(str::trim)
                        .map(str::to_owned)
                        .collect(),
                };

                arg_groups.push(CcTest::new(tag, &args[0]));
            }
        }
    }
    Ok(arg_groups)
}

pub fn find_tests_in_cc_source(test_source: &Path) -> anyhow::Result<HashSet<String>> {
    let mut tests = HashSet::new();
    let data = fs::read_to_string(test_source)?;
    let tags = [
        "FIXTURE_TEST",
        "SEASTAR_THREAD_TEST_CASE",
        "BOOST_AUTO_TEST_CASE",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    let tests_and_tags = parse_test_name_from_source(&data, &tags, SplitOn::Delim(","))?;
    for test in tests_and_tags {
        println!("found test {} of type: {}", test.name, test.tag);
        tests.insert(test.name);
    }
    Ok(tests)
}

pub fn collect_cmake_test_definitions(root: &str) -> anyhow::Result<Vec<RpTest>> {
    let mut collected_tests = Vec::new();
    for entry in WalkDir::new(root) {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() && path.file_name().unwrap().to_string_lossy() == "CMakeLists.txt" {
            let parent = path.parent().unwrap();
            if parent.file_name().unwrap() == "tests" {
                println!("collecting tests from {:?}", path);
                let mut tests = parse_tests_from_file(path)?;
                println!("found {} test suites", tests.len());
                for t in tests.iter_mut() {
                    for source in &t.sources {
                        let mut path = parent.to_owned();
                        path.push(source);
                        println!("looking for tests in {:?}", path);
                        let tests_in_file = find_tests_in_cc_source(&path)?;
                        t.tests.extend(tests_in_file.into_iter());
                        println!("found {} tests in {:?}", t.tests.len(), path);
                    }
                }
                collected_tests.extend(tests.into_iter());
            }
        }
    }

    Ok(collected_tests)
}
