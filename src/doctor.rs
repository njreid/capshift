//! Installation and service health checks for macOS.

use anyhow::{bail, Context, Result};
use std::{
    path::{Path, PathBuf},
    process::Command,
};

const DRIVER_CASK: &str = "njreid/capshift/karabiner-driverkit-virtualhiddevice";
const KVHD_LABEL: &str = "dev.njreid.capshift.kvhd";
const MENU_LABEL: &str = "dev.njreid.capshift-menu";
const KVHD_PLIST: &str = "dev.njreid.capshift.kvhd.plist";
const MENU_PLIST: &str = "dev.njreid.capshift-menu.plist";
const DRIVER_DAEMON: &str = "/Library/Application Support/org.pqrs/Karabiner-DriverKit-VirtualHIDDevice/Applications/Karabiner-VirtualHIDDevice-Daemon.app/Contents/MacOS/Karabiner-VirtualHIDDevice-Daemon";

pub fn run(fix: bool) -> Result<()> {
    println!("capshift doctor{}", if fix { " --fix" } else { "" });

    let resources = installed_resources()?;
    let mut healthy = true;

    if !cask_installed() {
        healthy = false;
        println!("✗ VirtualHID driver cask is not installed");
        if fix {
            println!("  installing {DRIVER_CASK}…");
            run_command("brew", ["install", "--cask", DRIVER_CASK])?;
        }
    } else {
        println!("✓ VirtualHID driver cask is installed");
    }

    if !Path::new(DRIVER_DAEMON).exists() {
        healthy = false;
        println!("✗ Karabiner VirtualHID daemon executable is missing");
    } else {
        println!("✓ Karabiner VirtualHID daemon executable is present");
    }

    let kvhd_destination = Path::new("/Library/LaunchDaemons").join(KVHD_PLIST);
    let kvhd_loaded = launchd_loaded(&format!("system/{KVHD_LABEL}"));
    if !kvhd_destination.exists() || !kvhd_loaded {
        healthy = false;
        println!("✗ VirtualHID root LaunchDaemon is not installed and loaded");
        if fix {
            install_kvhd(&resources.join(KVHD_PLIST), &kvhd_destination)?;
            println!("  repaired VirtualHID root LaunchDaemon");
        }
    } else {
        println!("✓ VirtualHID root LaunchDaemon is installed and loaded");
    }

    let daemon_loaded = launchd_loaded("system/homebrew.mxcl.capshift");
    if !daemon_loaded {
        healthy = false;
        println!("✗ capshift root service is not loaded");
        if fix {
            run_privileged("brew services start capshift")?;
            println!("  started capshift root service");
        }
    } else {
        println!("✓ capshift root service is loaded");
    }

    let menu_destination = user_launch_agents_dir()?.join(MENU_PLIST);
    let menu_domain = format!("gui/{}/{}", uid()?, MENU_LABEL);
    if !menu_destination.exists() || !launchd_loaded(&menu_domain) {
        healthy = false;
        println!("✗ menu-bar LaunchAgent is not installed and loaded");
        if fix {
            install_menu(&resources.join(MENU_PLIST), &menu_destination)?;
            println!("  repaired menu-bar LaunchAgent");
        }
    } else {
        println!("✓ menu-bar LaunchAgent is installed and loaded");
    }

    let config = Path::new(crate::config::SYSTEM_CONFIG_PATH);
    if !config.exists() {
        healthy = false;
        println!("✗ shared configuration is missing: {}", config.display());
        if fix {
            // Starting the root service creates the documented starter config.
            run_privileged("brew services restart capshift")?;
            println!("  started capshift so it can create its starter config");
        }
    } else {
        println!("✓ shared configuration exists: {}", config.display());
    }

    println!("! DriverKit extension approval cannot be automated. Check System Settings → Privacy & Security if the driver is not active.");
    println!("! Accessibility permission cannot be automated. Add capshift in System Settings → Privacy & Security → Accessibility.");

    if healthy && !fix {
        println!("All repairable capshift dependencies are healthy.");
    } else if !fix {
        println!("Run `capshift doctor --fix` to repair the items above.");
    }
    Ok(())
}

fn installed_resources() -> Result<PathBuf> {
    let output = Command::new("brew")
        .args(["--prefix", "capshift"])
        .output()
        .context("locating capshift through Homebrew")?;
    if !output.status.success() {
        bail!("capshift is not installed by Homebrew; cannot locate packaged service files");
    }
    let prefix =
        String::from_utf8(output.stdout).context("Homebrew returned a non-UTF-8 prefix")?;
    Ok(PathBuf::from(prefix.trim()).join("share/capshift"))
}

fn cask_installed() -> bool {
    Command::new("brew")
        .args(["list", "--cask", "karabiner-driverkit-virtualhiddevice"])
        .status()
        .is_ok_and(|status| status.success())
}

fn launchd_loaded(domain: &str) -> bool {
    Command::new("launchctl")
        .args(["print", domain])
        .status()
        .is_ok_and(|status| status.success())
}

fn install_kvhd(source: &Path, destination: &Path) -> Result<()> {
    ensure_resource(source)?;
    let command = format!(
        "install -m 644 {} {} && chown root:wheel {} && (launchctl bootout system/{} 2>/dev/null || true) && launchctl bootstrap system {}",
        shell_quote(source), shell_quote(destination), shell_quote(destination), KVHD_LABEL, shell_quote(destination)
    );
    run_privileged(&command)
}

fn install_menu(source: &Path, destination: &Path) -> Result<()> {
    ensure_resource(source)?;
    let parent = destination
        .parent()
        .expect("LaunchAgents path has a parent");
    let domain = format!("gui/{}", uid()?);
    let command = format!(
        "mkdir -p {} && install -m 644 {} {} && (launchctl bootout {}/{} 2>/dev/null || true) && launchctl bootstrap {} {}",
        shell_quote(parent), shell_quote(source), shell_quote(destination), domain, MENU_LABEL, domain, shell_quote(destination)
    );
    run_shell(&command)
}

fn user_launch_agents_dir() -> Result<PathBuf> {
    let home = std::env::var_os("HOME").context("HOME is not set")?;
    Ok(PathBuf::from(home).join("Library/LaunchAgents"))
}

fn uid() -> Result<String> {
    let output = Command::new("id")
        .arg("-u")
        .output()
        .context("getting current user ID")?;
    if !output.status.success() {
        bail!("could not get current user ID");
    }
    Ok(String::from_utf8(output.stdout)?.trim().to_owned())
}

fn ensure_resource(path: &Path) -> Result<()> {
    if path.exists() {
        Ok(())
    } else {
        bail!("packaged resource is missing: {}", path.display())
    }
}

fn run_command(program: &str, args: impl IntoIterator<Item = &'static str>) -> Result<()> {
    let status = Command::new(program)
        .args(args)
        .status()
        .with_context(|| format!("running {program}"))?;
    if status.success() {
        Ok(())
    } else {
        bail!("{program} exited with {status}")
    }
}

fn run_shell(command: &str) -> Result<()> {
    let status = Command::new("/bin/sh")
        .args(["-c", command])
        .status()
        .context("repairing menu LaunchAgent")?;
    if status.success() {
        Ok(())
    } else {
        bail!("menu LaunchAgent repair failed")
    }
}

fn run_privileged(command: &str) -> Result<()> {
    let script = format!(
        "do shell script \"{}\" with administrator privileges",
        applescript_escape(command)
    );
    let status = Command::new("osascript")
        .args(["-e", &script])
        .status()
        .context("starting macOS administrator prompt")?;
    if status.success() {
        Ok(())
    } else {
        bail!("administrator command failed")
    }
}

fn shell_quote(path: &Path) -> String {
    format!("'{}'", path.display().to_string().replace('\'', "'\\''"))
}

fn applescript_escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quotes_paths_for_shell_commands() {
        assert_eq!(
            shell_quote(Path::new("/Library/Application Support/capshift")),
            "'/Library/Application Support/capshift'"
        );
        assert_eq!(shell_quote(Path::new("a'b")), "'a'\\''b'");
    }
}
