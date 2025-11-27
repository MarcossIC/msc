use anyhow::{Context, Result};

pub fn execute(matches: &clap::ArgMatches) -> Result<()> {
    let name = matches
        .get_one::<String>("name")
        .context("Name argument is required")?;
    println!("Hello, {}!", name);
    Ok(())
}
