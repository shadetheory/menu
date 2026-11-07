# Future plugin shape

This is a design note from the 6:19–6:21 discussion. It is not implemented. The menu core stays a native Rust window plus the read-only `--list` and `--dry-run` commands.

A later Omarchy launcher can grow plugins without becoming a silent controller of the whole computer.

- A plugin may talk to the menu through a CLI or a small HTTP API. MCP is not required.
- Each capability is off until someone opts in. The menu can recommend a capability. It does not enable it because the plugin exists.
- A plugin does not receive search text inside a shell string. Actions stay argv, the same rule as desktop Exec and `--terminal-argv`.
- Inbox, groceries, and bot sessions stay unavailable until a documented interface exists that does not need a scraped secret.

No plugin process, socket, or account client is started by this tree. The Spotlight pill does not load a plugin framework. Music, bots, groceries, inbox, and store stay hidden until a later capability exists.
