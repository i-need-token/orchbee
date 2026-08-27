# `@orchbee/remote-client-protocol`

`@orchbee/remote-client-protocol` owns the strict, runtime-neutral contract for a human Browser to
exchange one Personal Access Token for an opaque Orchbee Browser session. The current consumer is
the opt-in Desktop Browser-access preview. HTTP and WebSocket adapters remain with their owning
applications.

## Authentication model

| Field        | Contract                                                                                                                                                                                                                                         |
| ------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Problem      | A Browser user must authenticate without putting a reusable credential in a URL or exposing it to same-origin script after login.                                                                                                                |
| Inputs       | One untrusted 43-character base64url Personal Access Token, an optional Cookie header, a same-origin login request, and an active Browser-access session-signing key. Absence means unauthenticated.                                             |
| Data         | Desktop owns one installation-scoped token verifier; one active Browser-access runtime owns one random signing key; each successful exchange creates one random opaque session identity.                                                         |
| Outputs      | A successful exchange returns an `HttpOnly`, `SameSite=Strict`, host-only session Cookie. Invalid input returns one bounded authentication error and creates no session.                                                                         |
| Invariants   | The raw Personal Access Token never enters a URL or Cookie; a Cookie is accepted only when its exact payload signature validates; duplicate session Cookies fail closed; a signing-key rotation invalidates every prior session.                 |
| Decisions    | Static renderer assets and the login document are public; product documents require a valid session; current HTTP preview Cookies omit `Secure` and retain an explicit unencrypted-LAN warning.                                                  |
| Model        | Strict schemas validate the exchange request and credentials; pure Cookie functions encode, extract, and verify one canonical signed session value.                                                                                              |
| Dependencies | Callers inject cryptographic signing, constant-time verification, randomness, bounded in-memory attempt limiting, HTTP response handling, and process lifecycle. This package reads no globals.                                                  |
| Ownership    | Desktop owns token creation, verifier persistence, and rotation. Server owns token verification and Cookie issuance. Gateway owns WebSocket admission from the same signed session.                                                              |
| Verification | Contract tests cover canonical and malformed tokens, credential tampering, duplicate Cookies, required attributes, and the optional future TLS attribute. Application tests cover cryptography, routing, rate limiting, and WebSocket admission. |

## Lifecycle and transitions

| State            | Event                                                               | Result                                                                                                                 |
| ---------------- | ------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------- |
| Token missing    | Create                                                              | Persist the verifier atomically and reveal the new token once.                                                         |
| Token configured | Reset                                                               | Stop active Browser access, persist a fresh verifier atomically, reveal the new token once, and require a fresh login. |
| Unauthenticated  | Submit matching token                                               | Create a fresh signed session Cookie and resume the retained same-origin product route.                                |
| Unauthenticated  | Submit malformed, mismatched, cross-origin, or rate-limited request | Remain unauthenticated and return a bounded error.                                                                     |
| Authenticated    | Reload product route                                                | Admit the document and Gateway control connection while the active signing key validates the Cookie.                   |
| Authenticated    | Stop access, reset token, or restart access                         | Rotate or discard the signing key, close Gateway connections, and reject the old Cookie.                               |

Token rotation commits at Desktop's atomic verifier-store replacement. Browser-access shutdown is a
precondition of reset. If shutdown or persistence fails, no new verifier is committed. If verifier
persistence commits but one-time renderer delivery fails, the token remains unrecoverable and the
user recovers by resetting it again.

## Correctness claims and limits

| Claim                                                                              | Evidence                                                                                 | Limit                                                                               |
| ---------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------- |
| Raw Personal Access Tokens do not become Browser session credentials.              | Cookie contract tests plus Server exchange integration.                                  | A trusted Desktop renderer sees a newly created token once so the user can copy it. |
| A forged, malformed, stale, or duplicated Cookie grants no Browser authority.      | Pure credential tests, constant-time adapter tests, and real HTTP/WebSocket integration. | Same-user process inspection and a compromised signing key are outside the claim.   |
| Reset and Browser-access lifecycle retirement revoke every active Browser session. | Desktop workflow tests and Gateway disconnect/reconnect integration.                     | The current preview has no remote logout propagation while the Server is stopped.   |
| An intended product route survives login without an open redirect.                 | Return-target policy tests and Browser login rendering in both shipped locales.          | Only same-origin relative product routes are supported.                             |

The current integration remains a loopback or explicit RFC 1918 trusted-LAN HTTP preview. It does
not claim confidentiality on the network, Internet safety, multi-user authorization, account
recovery, durable rate-limit state, TLS termination, or device authentication. Capability nodes use
separate non-Cookie credentials and are outside this package.

## Public API and verification

The root export provides the token, verifier, session identity, signing-key, exchange, error, route,
and Cookie schemas and helpers. It deliberately exposes no cryptographic or persistence adapter.

Run:

```sh
pnpm --filter @orchbee/remote-client-protocol build
pnpm --filter @orchbee/remote-client-protocol typecheck
pnpm --filter @orchbee/remote-client-protocol test
pnpm --filter @orchbee/remote-client-protocol test:exports
```
