use std::{fs, path::Path};

use anyhow::Result;

use self::{
    parsers::{dispatch_tag_parse, skip_to_next_tag},
    structures::{ParseContext, ParsedTag, RpTest},
};

mod lazy_binding;
mod parsers;
pub mod structures;

fn parse_unit(input: &str) -> ParseContext {
    let mut parse_ctx = ParseContext::default();
    let mut input = input;
    let mut tag;
    while !input.is_empty() {
        (input, tag) = skip_to_next_tag(input).expect("failed to skip to next tag");
        if tag == ParsedTag::EOF {
            break;
        }
        input = dispatch_tag_parse(input, &mut parse_ctx, tag)
            .expect("failed to dispatch tag parse")
            .0;
    }

    parse_ctx
}

pub fn parse_tests_from_file(p: &Path) -> Result<Vec<RpTest>> {
    let data = fs::read_to_string(p)?;
    let mut tests = Vec::new();
    let ctx = parse_unit(&data);
    for (_, test) in ctx.tests {
        tests.push(test);
    }
    Ok(tests)
}
