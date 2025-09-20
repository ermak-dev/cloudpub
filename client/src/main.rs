use anyhow::{Context, Result};
use clap::Parser;
use cloudpub_client::base::{cli_main, init, Cli};
use tracing::error;

pub fn main() -> Result<()> {
    // Check if we're being run as a service on Windows
    #[cfg(target_os = "windows")]
    {
        use std::env;
        if env::args().any(|arg| arg == "--run-as-service") {
            return cloudpub_client::service::run_as_service();
        }
    }

    let cli = Cli::parse();
    let (_guard, config) = init(&cli).context("Failed to initialize config")?;
    if let Err(err) = cli_main(cli, config) {
        error!("Exiting with error: {}", err);
        eprintln!("{}", err);
        std::process::exit(1);
    } else {
        Ok(())
    }
}
