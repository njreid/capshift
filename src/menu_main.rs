//! Entry point for capshift's per-user menu-bar companion.

#[cfg(target_os = "macos")]
#[path = "menubar.rs"]
mod menubar;

fn main() {
    #[cfg(target_os = "macos")]
    if let Err(error) = menubar::run() {
        eprintln!("capshift-menu: {error:#}");
        std::process::exit(1);
    }

    #[cfg(not(target_os = "macos"))]
    {
        eprintln!("capshift-menu only supports macOS");
        std::process::exit(1);
    }
}
