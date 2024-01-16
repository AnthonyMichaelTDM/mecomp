# MECOMP-Daemon

`mecomp-daemon` is the core of the MECOMP application. It is a long-running RPC server that handles all the backend logic and state-management necessary for the application to function. The MECOMP clients are simply frontends to this server.

This project contains two crates, `mecomp-daemon` and `mecomp-daemon-lib`. The `mecomp-daemon` crate is the binary crate, and the `mecomp-daemon-lib` crate is the library crate. The binary crate is the actual RPC server, and the library crate contains code that the MECOMP clients can use to communicate with the daemon.
