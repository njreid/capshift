use anyhow::Result;
use clap::{Parser, Subcommand};
#[cfg(target_os = "macos")]
use tracing::info;

mod actions;
mod apps;
mod chord;
mod config;
#[cfg(target_os = "macos")]
mod doctor;
mod keycodes;
mod kvhd;
#[cfg(target_os = "macos")]
mod menubar;

#[cfg(target_os = "macos")]
mod hid;

/// capshift — caps-lock chord shortcut daemon for macOS.
#[derive(Parser, Debug)]
#[command(version, about)]
struct Args {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// List running applications as name<TAB>bundle-id.
    #[command(alias = "applications")]
    Apps {
        /// Include background agents, helpers, and UI services.
        #[arg(long)]
        all: bool,
    },
    /// Run the per-user macOS menu-bar companion.
    Menu,
    /// Check capshift's macOS dependencies and startup services.
    Doctor {
        /// Install or repair everything that can be fixed automatically.
        #[arg(long)]
        fix: bool,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(std::env::var("RUST_LOG").unwrap_or_else(|_| "capshift=info".into()))
        .init();

    let args = Args::parse();

    match args.command {
        Some(Command::Apps { all }) => {
            apps::print_running(all)?;
            return Ok(());
        }
        Some(Command::Menu) => {
            #[cfg(target_os = "macos")]
            return menubar::run();
            #[cfg(not(target_os = "macos"))]
            anyhow::bail!("capshift-menu only supports macOS");
        }
        Some(Command::Doctor { fix }) => {
            #[cfg(target_os = "macos")]
            return doctor::run(fix);
            #[cfg(not(target_os = "macos"))]
            anyhow::bail!("capshift doctor only supports macOS");
        }
        None => {}
    }

    #[cfg(not(target_os = "macos"))]
    {
        eprintln!("error: capshift only supports macOS");
        std::process::exit(1);
    }

    #[cfg(target_os = "macos")]
    {
        let _ = std::fs::remove_file(hid::READY_FILE);
        let cfg_rx = config::watch()?;
        info!("config: {}", config::config_path()?.display());

        std::thread::spawn(move || {
            if let Err(e) = hid::run(cfg_rx) {
                tracing::error!("hid interceptor: {e}");
                std::process::exit(1);
            }
        });

        std::future::pending::<()>().await;
    }

    Ok(())
}
