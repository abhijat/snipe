use std::collections::HashSet;

use nom::{
    branch::alt,
    bytes::complete::{tag, take_till, take_while},
    character::{
        complete::{anychar, multispace0, multispace1},
        is_alphanumeric,
    },
    combinator::{eof, map},
    multi::{many_till, separated_list1},
    sequence::{delimited, separated_pair, terminated},
    IResult,
};

use crate::cmake_parser::lazy_binding::LazyBinding;

use super::structures::{ParseContext, ParsedTag, RpTest, SourceSet, TestKind};

pub(crate) fn skip_to_next_tag(input: &str) -> IResult<&str, ParsedTag> {
    let known_terms = alt((
        tag("set("),
        tag("set ("),
        tag("foreach("),
        tag("foreach ("),
        tag("endforeach()"),
        tag("endforeach ()"),
        tag("rp_test("),
        tag("rp_test ("),
        tag("get_filename_component("),
        tag("get_filename_component ("),
        eof,
    ));

    let drop_parser = map(anychar, drop);
    many_till(drop_parser, known_terms)(input)
        .map(|(rem, (_, res))| (rem, ParsedTag::from_str(res)))
}

fn parse_substitution(input: &str) -> IResult<&str, &str> {
    delimited(
        tag("${"),
        take_while(|c: char| is_alphanumeric(c as u8) || c == '_'),
        tag("}"),
    )(input)
}

pub(crate) fn parse_identifier(input: &str) -> IResult<&str, String> {
    let valid = "#${}_:.\"-/=";
    take_till(|c| !is_alphanumeric(c as u8) && !valid.contains(c))(input)
        .map(|(rem, res)| (rem, res.to_owned()))
}

pub(crate) fn parse_set_sources(input: &str) -> IResult<&str, SourceSet> {
    many_till(
        delimited(multispace0, parse_identifier, multispace0),
        tag(")"),
    )(input)
    .map(|(rem, (keys, _))| (rem, SourceSet::new(keys)))
}

fn find_test_name(tokens: &Vec<String>) -> String {
    let mut it = tokens.iter();
    let bin = it
        .position(|s| s == "BINARY_NAME")
        .unwrap_or_else(|| panic!("invalid tokens for rp_test {:?}", tokens));
    tokens[bin + 1].to_owned()
}

fn find_test_sources(tokens: &Vec<String>) -> HashSet<String> {
    let mut it = tokens.iter();
    let bin = it
        .position(|s| s == "SOURCES")
        .unwrap_or_else(|| panic!("invalid tokens for rp_test {:?}", tokens));
    let is_stop_word = |s: &str| s.chars().all(|c| c.is_uppercase() || c == '_');
    tokens
        .iter()
        .skip(bin + 1)
        .take_while(|token| !is_stop_word(token))
        .map(|s| s.to_owned())
        .collect()
}

fn parse_rp_test(input: &str) -> IResult<&str, RpTest> {
    let rp_test_body = terminated(separated_list1(multispace1, parse_identifier), tag(")"));
    let (rem, res) = delimited(multispace0, rp_test_body, multispace0)(input)?;

    let kind = match res[0].as_str() {
        "FIXTURE_TEST" => TestKind::Fixture,
        "UNIT_TEST" => TestKind::Unit,
        "BENCHMARK_TEST" => TestKind::Bench,
        _ => panic!("unexpected kind of test {:?}", res[0]),
    };

    Ok((
        rem,
        RpTest {
            name: find_test_name(&res),
            sources: find_test_sources(&res),
            kind,
            tests: Default::default(),
        },
    ))
}

fn parse_foreach<'a>(input: &'a str, ctx: &ParseContext) -> IResult<&'a str, Vec<RpTest>> {
    let (rem, (loop_var, input_arg)) =
        separated_pair(parse_identifier, multispace1, parse_substitution)(input)?;
    let (rem, _) = tag(")")(rem)?;

    let source_set = ctx
        .source_sets
        .get(input_arg)
        .expect(&format!("unexpected key {input_arg}"));

    let mut lazy_binding = LazyBinding::default();
    lazy_binding.add(&loop_var);

    let (mut rem, mut tag) = skip_to_next_tag(rem)?;
    if tag == ParsedTag::GetFileNameComponent {
        let result = parse_identifier(rem)?;
        rem = result.0;
        lazy_binding.add_transformed(&result.1, &loop_var, |v| v.replace(".Cc", ""));
        (rem, _) = take_till(|c| c == ')')(rem)?;
        (rem, tag) = skip_to_next_tag(&rem[1..])?;
    }

    assert!(tag == ParsedTag::RpTest, "unexpected tag {:?}", tag);
    let (rem, rp_test) = parse_rp_test(rem)?;

    let (rem, tag) = skip_to_next_tag(rem)?;
    assert!(tag == ParsedTag::EndForEach, "unexpected tag {:?}", tag);

    let mut tests = Vec::default();
    for source in &source_set.files {
        lazy_binding.populate(&loop_var, &source);
        let test = rp_test.eval(&lazy_binding.to_map());
        tests.push(test);
    }

    Ok((rem, tests))
}

pub(crate) fn dispatch_tag_parse<'a>(
    input: &'a str,
    parse_ctx: &mut ParseContext,
    tag: ParsedTag,
) -> IResult<&'a str, ()> {
    match tag {
        ParsedTag::Set => {
            let (input, source_set) = parse_set_sources(input)?;
            parse_ctx
                .source_sets
                .insert(source_set.name.clone(), source_set);
            Ok((input, ()))
        }
        ParsedTag::ForEach => {
            let (input, tests) = parse_foreach(input, &parse_ctx)?;
            for test in tests {
                parse_ctx.tests.insert(test.name.clone(), test);
            }
            Ok((input, ()))
        }
        ParsedTag::EndForEach => {
            panic!("a wild endforeach appeared");
        }
        ParsedTag::RpTest => {
            let (input, mut test) = parse_rp_test(input)?;
            if test.needs_source_expansion() {
                test.expand_sources(&parse_ctx);
            }
            parse_ctx.tests.insert(test.name.clone(), test);
            Ok((input, ()))
        }
        ParsedTag::GetFileNameComponent => {
            // Do nothing, we do not care about a getfilename... outside of a foreach
            Ok((input, ()))
        }
        ParsedTag::EOF => Ok((input, ())),
    }
}
