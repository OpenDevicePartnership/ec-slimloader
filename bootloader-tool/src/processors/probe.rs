use probe_rs::{
    Permissions, Session,
    probe::{DebugProbeSelector, list::Lister},
};

pub async fn start_session(chip: &str, probe_selector: Option<String>) -> anyhow::Result<Session> {
    let session = if let Some(ref probe) = probe_selector {
        Lister::new()
            .open(DebugProbeSelector::try_from(&**probe)?)
            .await?
    } else {
        let probes = Lister::new().list_all().await;
        let probe = match probes.len() {
            0 => return Err(anyhow::anyhow!("No probe found")),
            1 => probes.first().unwrap(),
            _ => {
                eprintln!("Use --probe to select one of the following available Probes:");
                for (i, probe_info) in probes.iter().enumerate() {
                    eprintln!("{i}: {probe_info}");
                }

                std::process::exit(1);
            }
        };

        probe.open().unwrap()
    }
    .attach(chip, Permissions::default())?;

    Ok(session)
}
