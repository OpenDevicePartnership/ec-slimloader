extern crate log;
extern crate pretty_env_logger;

use anyhow::Context;
use bootloader_tool::{Cli, Config, commands};
use clap::Parser;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    pretty_env_logger::init();

    let cli = Cli::parse();

    let config = Config::read(&cli.config)
        .with_context(|| format!("Tried to open --config {}", cli.config.display()))?;

    if let Some(command) = cli.commands {
        commands::process(&config, command).await
    } else {
        eprintln!("Done nothing");
        Ok(())
    }
}
