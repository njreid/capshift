//! A small per-user status item for the root capshift LaunchDaemon.
//!
//! LaunchDaemons run outside a logged-in graphical session, so this companion
//! intentionally runs as a LaunchAgent. Root-only operations use macOS's
//! standard administrator-authentication prompt.

use anyhow::{Context, Result};
use cocoa::{
    appkit::{
        NSApp, NSApplication, NSApplicationActivationPolicyAccessory, NSMenu, NSMenuItem,
        NSStatusBar, NSStatusItem, NSVariableStatusItemLength,
    },
    base::{id, nil, YES},
    foundation::{NSAutoreleasePool, NSString},
};
use objc::runtime::{Object, Sel};
use objc::{class, declare::ClassDecl, msg_send, sel, sel_impl};
use std::process::Command;
use std::sync::Once;

const DAEMON_LABEL: &str = "homebrew.mxcl.capshift";
const DRIVER_LABEL: &str = "dev.njreid.capshift.kvhd";
const CONFIG_PATH: &str = "/Library/Application Support/capshift/config.kdl";

static REGISTER_HANDLER: Once = Once::new();
static mut STATUS_ITEM: id = nil;

pub fn run() -> Result<()> {
    unsafe {
        let _pool = NSAutoreleasePool::new(nil);
        register_handler();

        let app = NSApp();
        app.setActivationPolicy_(NSApplicationActivationPolicyAccessory);

        let status_bar = NSStatusBar::systemStatusBar(nil);
        let status_item = status_bar.statusItemWithLength_(NSVariableStatusItemLength);
        STATUS_ITEM = status_item;
        refresh_status();

        let handler: id = msg_send![class!(CapshiftMenuHandler), new];
        let menu = NSMenu::new(nil).autorelease();
        menu.addItem_(menu_item("Reload Driver", sel!(reloadDriver:), handler));
        menu.addItem_(menu_item("Restart Daemon", sel!(restartDaemon:), handler));
        menu.addItem_(menu_item("Edit Config", sel!(editConfig:), handler));
        menu.addItem_(NSMenuItem::separatorItem(nil));
        menu.addItem_(menu_item("Quit Capshift Menu", sel!(terminate:), app));
        status_item.setMenu_(menu);

        // Keep the icon truthful even when the root daemon exits or is restarted
        // outside this menu.
        let _: id = msg_send![class!(NSTimer), scheduledTimerWithTimeInterval: 5.0f64
            target: handler selector: sel!(refreshStatus:) userInfo: nil repeats: YES];

        app.run();
    }
    Ok(())
}

unsafe fn menu_item(title: &str, action: Sel, target: id) -> id {
    let item = NSMenuItem::alloc(nil).initWithTitle_action_keyEquivalent_(
        NSString::alloc(nil).init_str(title),
        action,
        NSString::alloc(nil).init_str(""),
    );
    item.setTarget_(target);
    item
}

unsafe fn refresh_status() {
    if STATUS_ITEM == nil {
        return;
    }
    let item: id = STATUS_ITEM;
    let button: id = msg_send![item, button];
    if button == nil {
        return;
    }
    let running = daemon_is_running();
    let title = if running { "⇪" } else { "⇪!" };
    let tooltip = if running {
        "capshift is running"
    } else {
        "capshift is not running"
    };
    let _: () = msg_send![button, setTitle: NSString::alloc(nil).init_str(title)];
    let _: () = msg_send![button, setToolTip: NSString::alloc(nil).init_str(tooltip)];
}

fn daemon_is_running() -> bool {
    // A GUI LaunchAgent cannot reliably inspect the root launchd domain.
    // launchd adopts a LaunchDaemon directly, giving it parent PID 1; this
    // deliberately excludes a manually-run `sudo capshift` in a Terminal.
    Command::new("ps")
        .args(["-axo", "ppid=,comm="])
        .output()
        .is_ok_and(|output| {
            output.status.success()
                && String::from_utf8_lossy(&output.stdout).lines().any(|line| {
                    line.split_whitespace().next() == Some("1")
                        && (line.ends_with("/capshift") || line.ends_with(" capshift"))
                })
        })
}

unsafe fn register_handler() {
    REGISTER_HANDLER.call_once(|| {
        let superclass = class!(NSObject);
        let mut decl = ClassDecl::new("CapshiftMenuHandler", superclass)
            .expect("CapshiftMenuHandler class name must be unused");
        decl.add_method(
            sel!(reloadDriver:),
            reload_driver as extern "C" fn(&Object, Sel, id),
        );
        decl.add_method(
            sel!(restartDaemon:),
            restart_daemon as extern "C" fn(&Object, Sel, id),
        );
        decl.add_method(
            sel!(editConfig:),
            edit_config as extern "C" fn(&Object, Sel, id),
        );
        decl.add_method(
            sel!(refreshStatus:),
            refresh_status_timer as extern "C" fn(&Object, Sel, id),
        );
        decl.register();
    });
}

extern "C" fn reload_driver(_: &Object, _: Sel, _: id) {
    launch_privileged(
        format!("launchctl kickstart -k system/{DRIVER_LABEL}"),
        "VirtualHID driver reloaded",
    );
}

extern "C" fn restart_daemon(_: &Object, _: Sel, _: id) {
    launch_privileged(
        format!("launchctl kickstart -k system/{DAEMON_LABEL}"),
        "capshift daemon restarted",
    );
}

extern "C" fn edit_config(_: &Object, _: Sel, _: id) {
    // Terminal may be configured to start fish (or another non-POSIX shell).
    // Run the command through /bin/sh so the ${VAR:-default} expansions are
    // reliable, while preserving EDITOR and SHELL from the terminal session.
    // `sudo` provides the write permission for the system-wide configuration.
    let command = format!(
        "/bin/sh -lc 'sudo \"${{EDITOR:-vi}}\" \"{CONFIG_PATH}\"; exec \"${{SHELL:-/bin/zsh}}\" -l'"
    );
    let script = format!("tell application \"Terminal\" to do script \"{}\"\ntell application \"Terminal\" to activate", applescript_escape(&command));
    let _ = Command::new("osascript").args(["-e", &script]).status();
}

extern "C" fn refresh_status_timer(_: &Object, _: Sel, _: id) {
    unsafe { refresh_status() };
}

/// Never block or unwind through an AppKit selector callback. `osascript`
/// displays an administrator prompt and can wait indefinitely for user input;
/// doing that on the menu bar's main thread makes macOS treat the agent as
/// unresponsive. The repeating status timer updates the icon after completion.
fn launch_privileged(command: String, success_message: &'static str) {
    std::thread::spawn(move || match run_privileged(&command) {
        Ok(()) => show_notification(success_message),
        Err(error) => {
            eprintln!("capshift-menu: {error:#}");
            show_notification(&format!("Operation failed: {error:#}"));
        }
    });
}

fn show_notification(message: &str) {
    let script = format!(
        "display notification \"{}\" with title \"capshift\"",
        applescript_escape(message)
    );
    let _ = Command::new("osascript").args(["-e", &script]).status();
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
    anyhow::ensure!(status.success(), "administrator command failed");
    Ok(())
}

fn applescript_escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}
