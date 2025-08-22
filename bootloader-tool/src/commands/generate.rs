use crate::{GenerateCommands, config::Config, processors};

pub async fn process(config: &Config, command: GenerateCommands) -> anyhow::Result<()> {
    match command {
        GenerateCommands::Certificates(args) => processors::certificates::generate(args, config),
        GenerateCommands::Otp => {
            let _ = processors::otp::generate(config)?;
            Ok(())
        }
    }
}
