use std::collections::HashMap;
use std::env;
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};

use anyhow::{anyhow, Result};
use handlebars::{no_escape, Handlebars};
use serde_json::json;

use crate::cmake_parser::structures::{RpTest, TestKind};
use crate::config::CommandRunConfig;
use crate::parse_env_file;
use crate::py_parser::ClassWithTests;

fn load_build_type() -> String {
    let default = "DEBUG".to_owned();
    let map = parse_env_file();
    if map.is_err() {
        return default;
    }

    map.unwrap()
        .get("BUILD_TYPE")
        .unwrap_or(&default)
        .to_owned()
}

fn build_cc_command(
    test: RpTest,
    test_name: String,
    command_config: &CommandRunConfig,
) -> Result<Vec<String>> {
    let mut h = Handlebars::new();
    h.register_escape_fn(no_escape);

    for (k, v) in &command_config.command_mappings {
        h.register_template_string(k, v)?;
    }
    let mut commands = Vec::with_capacity(h.get_templates().len());

    let build_type = load_build_type();
    let test_obj = match test.kind {
        TestKind::Unit => format!("{}_rpunit", test.name),
        TestKind::Fixture => format!("{}_rpfixture", test.name),
        TestKind::Bench => format!("{}_rpbench", test.name),
    };

    commands.push(h.render(
        "compile",
        &json!({
            "build_type": build_type,
            "test_obj": test_obj,
        }),
    )?);

    let test_tag_arg = format!("-t {test_name}");
    let pwd = env::current_dir()?;
    commands.push(h.render(
        "run",
        &json!({
                "build_type": build_type,
                "test_obj": test_obj,
                "test_tag_arg": test_tag_arg,
                "pwd": pwd.to_string_lossy(),
        }),
    )?);
    Ok(commands)
}

fn build_py_command(
    test: ClassWithTests,
    test_name: String,
    command_config: &CommandRunConfig,
) -> Result<Vec<String>> {
    let mut h = Handlebars::new();
    h.register_escape_fn(no_escape);
    for (k, v) in &command_config.command_mappings {
        h.register_template_string(k, v)?;
    }
    let mut commands = Vec::with_capacity(h.get_templates().len());

    let test_path = format!(
        "{}::{}.{}",
        test.source_path.to_string_lossy(),
        test.class_name,
        test_name
    );

    commands.push(h.render(
        "duck",
        &json!({
            "test_path": test_path,
            "test_args": "--repeat=1",
        }),
    )?);

    Ok(commands)
}

pub fn run_cc_test(
    test: RpTest,
    test_name: String,
    edit: bool,
    command_config: &CommandRunConfig,
    envs: &HashMap<String, String>,
) -> Result<()> {
    run_shell_commands(
        build_cc_command(test, test_name, command_config)?,
        edit,
        envs,
    )
}

pub fn run_py_test(
    test: ClassWithTests,
    test_name: String,
    edit: bool,
    command_config: &CommandRunConfig,
    envs: &HashMap<String, String>,
) -> Result<()> {
    run_shell_commands(
        build_py_command(test, test_name, command_config)?,
        edit,
        envs,
    )
}

fn edit_commands(commands: Vec<String>) -> Result<Vec<String>> {
    let mut editor = rustyline::DefaultEditor::new()?;
    let mut new_commands = Vec::with_capacity(commands.len());
    for command in commands {
        let edited = editor.readline_with_initial("Edit command >> ", (&command, ""))?;
        new_commands.push(edited);
    }
    Ok(new_commands)
}

fn run_shell_commands(
    commands: Vec<String>,
    edit: bool,
    envs: &HashMap<String, String>,
) -> Result<()> {
    let commands = if edit {
        edit_commands(commands)?
    } else {
        commands
    };

    for command_str in commands {
        let command_str = format!("-s -- {command_str}");
        let tokens = shell_words::split(&command_str)?;
        let mut command = Command::new("teetty")
            .args(tokens)
            .stdout(Stdio::piped())
            .envs(envs)
            .spawn()?;
        let o = command
            .stdout
            .as_mut()
            .ok_or_else(|| anyhow!("failed to get command output for: {command_str}"))?;

        for line in BufReader::new(o).lines() {
            match line {
                Ok(line) => println!("{}", line),
                Err(err) => {
                    println!("failed to run command {command_str}: ");
                    println!("{err}");
                    return Ok(());
                }
            }
        }
        command.wait()?;
    }
    Ok(())
}
