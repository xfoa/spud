# mDNS Discovery

Spud uses DNS-SD (RFC 6763) over mDNS for LAN service discovery. The `mdns-sd`
crate provides the underlying multicast DNS stack.

## Service type

| Name              | Value                 |
|-------------------|-----------------------|
| Service type      | `_spud._tcp.local.`   |

Spud advertises and browses for this service type only. Both TCP and UDP data
planes run on the port advertised in the mDNS SRV record.

## Instance naming

Server instances are named `{name}-{port}-{pid}` so multiple servers on the
same host do not collide. The `name` is the user-configured display name.

## TXT properties

Resolved service records carry the following TXT properties:

| Property | Value                                          |
|----------|------------------------------------------------|
| `name`   | User-configured display name.                  |
| `icon`   | One of `desktop`, `laptop`, `server`.          |
| `auth`   | `true` if the server requires authentication.  |
| `encrypt`| `true` if the server encrypts the UDP plane.   |

The client reads these properties to populate the discovered-server grid
(`DiscoveredServer`). The `icon` value is mapped to a FontAwesome glyph for
rendering.

## Wire events

The discovery module exposes a single `iced::futures::Stream` via
`discovery::browse()`:

```rust
pub enum Event {
    Found(DiscoveredServer),
    Lost(String),   // fullname
}
```

`Found` is emitted when a service is resolved (hostname and port are known).
`Lost` is emitted when a service is removed from the network. The client view
maintains a `Vec<DiscoveredServer>` and applies these events directly:

* `Found` — remove any existing entry with the same address, then insert and
  re-sort by name.
* `Lost` — remove the entry whose `fullname` matches.

## Server registration

`Registration` wraps an `mdns_sd` registration. It is created when the server
starts and unregistered on `Drop`. The server uses
`hostname.gethostname().local.` as the target host and calls
`enable_addr_auto()` so the mDNS daemon automatically publishes the local IP
addresses.

## Same-machine filtering

The server view checks `owns_fullname()` before forwarding `Found` events to
the client view. This prevents a server from listing itself in its own
discovery grid.
