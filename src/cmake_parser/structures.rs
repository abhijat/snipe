use std::collections::{HashMap, HashSet};
use std::fmt;
use std::fmt::{Display, Formatter};

use regex::Regex;
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, PartialEq)]
pub(crate) struct SourceSet {
    pub name: String,
    pub files: HashSet<String>,
}

impl SourceSet {
    pub fn new(keys: Vec<String>) -> Self {
        SourceSet {
            name: keys[0].clone(),
            files: keys.into_iter().skip(1).collect(),
        }
    }
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub enum TestKind {
    Unit,
    Fixture,
    Bench,
}

impl Display for TestKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            TestKind::Unit => write!(f, "Unit"),
            TestKind::Fixture => write!(f, "Fixture"),
            TestKind::Bench => write!(f, "Bench"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpTest {
    pub name: String,
    pub sources: HashSet<String>,
    pub kind: TestKind,
    pub tests: HashSet<String>,
}

impl Display for RpTest {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({})", self.name, self.kind)
    }
}

impl RpTest {
    pub(crate) fn eval(&self, variables: &HashMap<String, String>) -> RpTest {
        let mut clone = self.clone();
        clone.name = self.eval_name(variables);
        clone.sources = self.eval_source_list(variables);
        clone
    }

    pub(crate) fn eval_source_list(&self, variables: &HashMap<String, String>) -> HashSet<String> {
        let mut sources: HashSet<_> = Default::default();
        for src in &self.sources {
            if src.starts_with("${") && src.ends_with("}") {
                let variable: String = src.chars().skip(2).take(src.len() - 3).collect();
                let value = variables
                    .get(&variable)
                    .expect(&format!("variable {variable} is not in context"));
                sources.insert(value.to_owned());
            } else {
                sources.insert(src.to_owned());
            }
        }
        sources
    }

    pub(crate) fn eval_name(&self, variables: &HashMap<String, String>) -> String {
        let expr = Regex::new(r"\$\{.*\}").expect("bad regex!");
        let vars: Vec<_> = expr
            .find_iter(&self.name)
            .map(|m| {
                m.as_str()
                    .chars()
                    .skip(2)
                    .take(m.len() - 3)
                    .collect::<String>()
            })
            .collect();
        let mut name = self.name.clone();
        for var in vars {
            let value = variables
                .get(&var)
                .expect(&format!("missing variable {var}"));
            name = name.replace(&format!("${{{var}}}"), value);
        }
        name
    }

    pub(crate) fn needs_source_expansion(&self) -> bool {
        self.sources
            .iter()
            .map(|s| s.contains("$"))
            .fold(false, |acc, x| acc || x)
    }

    pub(crate) fn expand_sources(&mut self, ctx: &ParseContext) {
        let mut sources = HashSet::default();
        for src in &self.sources {
            if src.starts_with("${") && src.ends_with("}") {
                let variable: String = src.chars().skip(2).take(src.len() - 3).collect();
                let source_set = ctx
                    .source_sets
                    .get(&variable)
                    .unwrap_or_else(|| panic!("{variable} not in source set"));
                for file in &source_set.files {
                    sources.insert(file.to_owned());
                }
            } else {
                sources.insert(src.to_owned());
            }
        }
        self.sources = sources;
    }
}

#[derive(Debug, Default)]
pub(crate) struct ParseContext {
    pub source_sets: HashMap<String, SourceSet>,
    pub tests: HashMap<String, RpTest>,
}

#[derive(Debug, PartialEq)]
pub(crate) enum ParsedTag {
    Set,
    ForEach,
    EndForEach,
    RpTest,
    GetFileNameComponent,
    EOF,
}

impl ParsedTag {
    pub fn from_str(s: &str) -> Self {
        match s {
            "set(" => ParsedTag::Set,
            "set (" => ParsedTag::Set,
            "foreach(" => ParsedTag::ForEach,
            "foreach (" => ParsedTag::ForEach,
            "endforeach()" => ParsedTag::EndForEach,
            "endforeach ()" => ParsedTag::EndForEach,
            "rp_test(" => ParsedTag::RpTest,
            "rp_test (" => ParsedTag::RpTest,
            "get_filename_component(" => ParsedTag::GetFileNameComponent,
            "get_filename_component (" => ParsedTag::GetFileNameComponent,
            "" => ParsedTag::EOF,
            _ => panic!("unexpected match {s}"),
        }
    }
}
