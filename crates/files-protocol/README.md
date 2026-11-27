# File Bar Protocol

Private Rust wire contracts for the File Bar. `orchbee-server` and `orchbee-files` are its
consumers. It owns the identities File Bar traffic names and OBFL, the private link between a
Server and the `orchbee-files` process that Server admitted. It never opens a connection, chooses
an endpoint, compares peer credentials, runs a file operation, or decides scope; a decoded value
grants no authority.

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
