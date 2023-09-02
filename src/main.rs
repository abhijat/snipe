use anyhow::Result;
use clap::Parser;

use snipe::{Cli, SearchAndExecute};

fn main() -> Result<()> {
    let context = SearchAndExecute::from(Cli::parse());
    context.ensure_db_exists()?;
    context.find_test().and_then(|test| context.run_test(test))
}
