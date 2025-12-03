# MECOMP CLI

`mecomp-cli` is the command-line interface for the MECOMP application. It is a simple client that communicates with the MECOMP daemon over RPC to perform various tasks.
The CLI is designed to be user-friendly and easy to use, with a focus on simplicity and efficiency.

## Piping Support

The CLI supports piping output from one command to another, making it easy to chain operations together. When piping data, commands will automatically detect and read from stdin, so you don't need to use explicit `pipe` subcommands.

### Examples

**Queue operations:**
```sh
# Search for songs and add them to the queue
mecomp-cli search all "the beatles" -q | mecomp-cli queue add

# The old explicit pipe syntax still works but is deprecated
mecomp-cli search all "the beatles" -q | mecomp-cli queue pipe
```

**Playlist operations:**
```sh
# Search for songs and add them to a playlist
mecomp-cli search all "jazz" -q | mecomp-cli playlist add song <playlist-id>

# The old explicit pipe syntax still works but is deprecated
mecomp-cli search all "jazz" -q | mecomp-cli playlist add pipe <playlist-id>
```

**Radio operations:**
```sh
# Get similar songs based on search results
mecomp-cli search all "rock" -q | mecomp-cli radio song 10

# The old explicit pipe syntax still works but is deprecated
mecomp-cli search all "rock" -q | mecomp-cli radio pipe 10
```

The `-q` (quiet) flag in search commands outputs only record IDs, which is perfect for piping to other commands.
