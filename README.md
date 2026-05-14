# Spud

## WARNING: This project is a work in progress and is currently unusable

![The Spud icon, a potato with a game controller D-pad and buttons carved into it](resources/icon.png)

Spud solves the problem of wanting to play games on a computer attached your TV, from a laptop on your couch.
It's a cross-platform remote control application that sends local input to a remote server, and is optimised for gaming, meaning that input is as low latency as possible.
As it's intended to be used in a situation where you can already see the output from the server on another device, it doesn't accept video or sound output from the server.

## Why use Spud?

There already exist tools that solve similar problems to Spud, so why not use them instead?

* [Synergy](https://symless.com/synergy) is a great tool for remote control, but its latency makes it unsuitable for gaming.
* [Parsec](https://parsec.app/) is fantastically optimised and easy to set up, but it's too heavyweight when you don't need video sent back to the controlling device.
* [Moonlight/Sunlight](https://moonlight-stream.org/) are brilliant projects, but can be difficult to set up and don't support specific features of this use case.


## Features

* Simple UI
* Cross-platform
* Low latency input streaming
* Input capture toggling by hotkey or window focus
* Screen blanking on input capture
* Tolerance of poor network conditions
* Local screen blanking on client while capturing input
* Optional password protection and encryption
* LAN discovery

## Install

Binaries are available from the GitHub project [releases](https://github.com/xfoa/spud/releases) page.
Put them wherever your system expects to find binaries.

On **Linux**, the server can install input events via a privileged helper that
runs through `pkexec`. To avoid typing your password every time the server
starts, install the polkit rule:

```bash
sudo install -Dm644 resources/50-spud-injection.pkla \
    /etc/polkit-1/localauthority/50-local.d/50-spud-injection.pkla
```

Alternatively, run the provided install script which also builds and installs
the binary and desktop entry:

```bash
./install.sh
```

## Build

You can build this project using:

```
cargo build
```

Then run using:

```
cargo run
```

## Screenshots

### Client

![Spud client tab showing the connection page](resources/client-screenshot.png)

### Server

![Spud server tab showing the status page](resources/server-screenshot.png)

## Contribute

Please feel free to [open issues](https://github.com/xfoa/spud/issues), create [PRs](https://github.com/xfoa/spud/pulls), and [fork](https://github.com/xfoa/spud/fork) this project!

## License

This project is distributed under the [GPL-3.0](https://www.gnu.org/licenses/gpl-3.0.en.html) license.