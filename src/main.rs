use anyhow::Result;
use clap::Parser;

use snipe::{Cli, SearchAndExecute};

fn main() -> Result<()> {
    let cli = Cli::parse();
    if let Some(command_line) = cli.cli_content {
        SearchAndExecute::autocomplete(&command_line);
        Ok(())
    } else {
        let context = SearchAndExecute::from(cli);
        context.ensure_db_exists()?;
        context.find_test().and_then(|test| context.run_test(test))
    }
}
