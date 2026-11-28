# File Bar Protocol

Rust wire contracts for the File Bar. `orchbee-server`, `orchbee-files` and the OBWP multiplex of
`orchbee-workbench-protocol` are its consumers. It owns the Rust binding of the [File Bar contract](../../schemas/files/README.md), OBFB records and
the JSON of every `files` and `files-edit` operation, and OBFL, the private link between a Server
and the `orchbee-files` process that Server admitted. It never opens a connection, chooses an endpoint,
compares peer credentials, runs a file operation, or decides scope; a decoded value grants no
authority.

## Client records

A value that decodes is valid wherever the protocol carries it: decoding enforces the schema, the
UTF-8 byte bounds the schema cannot state, and the canonical shape, so an array where an object
belongs or `null` where a member may only be absent is refused even though `serde` alone would
read it. Request parsing tells an operation this build does not serve apart from a malformed
request, because the two close a stream differently, and a parsed request names the capability it
needs, which a Server checks against the connection's handshake answer. A record that only passes
through can be checked without copying it, as the multiplex checks each OBFB record of a carrier
that negotiated `files`. Whether a path lies inside a Workspace's scope is decided by
`orchbee-files` when the operation starts.

## The private link

OBFL has no compatibility window. A Server starts `orchbee-files` only from its own verified
executable cohort, so both ends always come from one build. The started process's hello names its
source commit, environment and the development fingerprint of the Client-Server protocol inputs; a
Server refuses any hello whose build differs instead of negotiating, and a malformed frame ends the
link. A version change therefore replaces the link atomically with the cohort.

Every kind has exactly one sender, so the codec is asked which end sent a frame and refuses a kind
the other end owns. The link never carries a path the Server built from a Client record: relayed
File Bar records travel unchanged inside the envelope, and the only paths the Server sends are the
Workspace roots its own Navigation holds.

The hello token is compared in constant time because the endpoint is reachable by any process of
the same user while the start is pending. The token is a capability for one start only; it is never
logged and never printed by `Debug`.
