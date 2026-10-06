# `@orchbee/remote-client-protocol`

Retained strict, runtime-neutral schemas and pure Cookie helpers for exchanging a Personal Access
Token for an opaque Browser session. The legacy Desktop Browser-access, Server, and Gateway
consumers are retired. This package has no current application consumer and is outside the six
retired workspaces; its presence does not provide Browser access or authentication.

## Public contract

The root export owns:

- bounded Personal Access Token, verifier, session identity, and signing-key schemas;
- strict authentication request and error schemas and the legacy route constants;
- versioned signed session-credential creation and verification through injected callbacks;
- Cookie extraction that rejects duplicate or empty session Cookies; and
- canonical Cookie serialization with `HttpOnly`, `SameSite=Strict`, and an explicit `Secure` option.

Credential helpers validate the session identity and signature shape. Cryptographic signing and
verification remain the caller's responsibility. The package owns no token store, login UI, HTTP
listener, WebSocket admission, rate limiter, or process lifecycle.

## Correctness and limits

Contract tests cover canonical and malformed tokens, credential format and tampering, injected
signature verification, duplicate Cookies, and required Cookie attributes. They establish the
library contract only. The retired applications' token rotation, login routing, network admission,
and session-revocation workflows are no longer current integration evidence.

## Verification

```sh
pnpm --filter @orchbee/remote-client-protocol build
pnpm --filter @orchbee/remote-client-protocol typecheck
pnpm --filter @orchbee/remote-client-protocol test
pnpm --filter @orchbee/remote-client-protocol test:exports
```
