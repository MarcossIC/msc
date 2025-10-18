use anyhow::Result;

pub fn execute(matches: &clap::ArgMatches) -> Result<()> {
    let name = matches.get_one::<String>("name").unwrap();
    println!("Hello, {}!", name);
    Ok(())
}
