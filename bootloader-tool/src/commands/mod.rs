mod download;
mod generate;
mod run;
mod sign;

use crate::{Commands, config::Config};

pub async fn process(config: &Config, command: Commands) -> anyhow::Result<()> {
    match command {
        Commands::Generate { subcommand } => generate::process(config, subcommand).await,
        Commands::Sign { subcommand } => {
            let _ = sign::process(config, subcommand).await?;
            Ok(())
        }
        Commands::Download { subcommand } => {
            download::process(config, subcommand).await?;
            Ok(())
        }
        Commands::Run { subcommand } => run::process(config, subcommand).await,
        Commands::Fuse => todo!(),
    }
}
