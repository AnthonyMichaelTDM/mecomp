# MECOMP-Daemon

`mecomp-daemon` is the core of the MECOMP application. It is a long-running RPC server that handles all the backend logic and state-management necessary for the application to function. The MECOMP clients are simply frontends to this server.

## Feature Flags

- `cli`: used when building the daemon binary
- `dynamic_updates`: when enabled, the daemon will use the `notify` crate to listen for changes to files in the users library and update the database accordingly

by default, the `cli` and `dynamic_updates` features are both enabled.

