use anyhow::Result;

pub fn execute() -> Result<()> {
    println!("msc version {}", env!("CARGO_PKG_VERSION"));
    Ok(())
}
