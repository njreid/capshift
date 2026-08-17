# capshift

`capshift` turns Caps Lock into a lightweight chord modifier on Apple Silicon
Macs. A chord can launch/focus an app, run a shell command, or emit another
key. It uses the Karabiner VirtualHID driver to forward keyboard reports.

## Install and start at boot

```sh
brew tap njreid/capshift
brew install --cask njreid/capshift/karabiner-driverkit-virtualhiddevice
brew install capshift

# Install the driver helper as a root startup service.
sudo cp /opt/homebrew/opt/capshift/share/capshift/dev.njreid.capshift.kvhd.plist /Library/LaunchDaemons/
sudo chown root:wheel /Library/LaunchDaemons/dev.njreid.capshift.kvhd.plist
sudo chmod 644 /Library/LaunchDaemons/dev.njreid.capshift.kvhd.plist
sudo launchctl bootstrap system /Library/LaunchDaemons/dev.njreid.capshift.kvhd.plist

# capshift itself is a root LaunchDaemon and therefore starts at every boot.
sudo brew services start capshift

# The optional menu companion belongs to your graphical login session.
cp /opt/homebrew/opt/capshift/share/capshift/dev.njreid.capshift-menu.plist ~/Library/LaunchAgents/
launchctl bootstrap gui/$(id -u) ~/Library/LaunchAgents/dev.njreid.capshift-menu.plist
```

Approve the DriverKit extension when macOS asks, and add `capshift` to
**System Settings → Privacy & Security → Accessibility**.

The menu icon is `⇪` while the daemon is running and `⇪!` otherwise. Its menu
can reload the VirtualHID driver, restart the root daemon, and open the shared
configuration in Terminal using `EDITOR` (with an administrator prompt).

### Check and repair installation

Use the doctor command after installation or an upgrade:

```sh
capshift doctor
capshift doctor --fix
```

`--fix` installs the VirtualHID cask if needed, repairs and reloads the root
VirtualHID LaunchDaemon, starts the root capshift service, and installs the
menu LaunchAgent for the logged-in user. It asks for administrator permission
only for system-level actions. macOS still requires you to approve the
DriverKit extension and grant Accessibility permission yourself; doctor
reports those requirements but cannot bypass them. It opens the exact
Accessibility pane and prints the installed `capshift` and `capshift-menu`
paths to add.

## Configuration

The root daemon always reads the system-wide file
`/Library/Application Support/capshift/config.kdl`. It creates an explained
starter file on first launch. Edit it through the menu item, or create it
before starting the daemon:

```sh
sudo mkdir -p "/Library/Application Support/capshift"
sudo "${EDITOR:-vi}" "/Library/Application Support/capshift/config.kdl"
```

Here is a complete example. Changes are applied automatically when saved.

```kdl
// Hold Caps Lock, then press the source key in each binding.
// app= focuses an already-running application or launches it by bundle ID.
bind "s" app="com.tinyspeck.slackmacgap" label="Slack"
bind "m" app="com.apple.mail" label="Mail"

// shell= runs the command with /bin/sh -c. label= is required for actions.
bind "t" shell="open -a Terminal" label="Terminal"

// key= emits another HID key. These make Caps+h/j/k/l navigation keys.
bind "h" key="left"
bind "j" key="down"
bind "k" key="up"
bind "l" key="right"

// mod= selects a binding only when those additional modifiers are held.
// Accepted names: shift, control (or ctrl), option (or alt), command (or cmd).
// Combine modifiers with +. The trigger modifiers are consumed for key remaps.
bind "h" mod="shift" key="home"
bind "l" mod="shift" key="end"
bind "j" mod="command+shift" shell="open -a Notes" label="Notes"
```

A bare Caps Lock press is consumed. Caps+Delete emits a normal Caps Lock
keystroke when you need to toggle it.

List bundle identifiers for foreground applications with:

```sh
capshift apps
capshift apps --all  # include helpers and background services
```

## Development

```sh
make test
make check
make build
```

Releases are created by pushing a `v*` tag. GitHub Actions builds
`aarch64-apple-darwin`, publishes the archive, and updates
`njreid/homebrew-capshift`. Configure the repository secret
`HOMEBREW_TAP_TOKEN` with write access to that tap before the first release.
