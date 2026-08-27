# `@orchbee/server`

`@orchbee/server` is the runnable HTTP renderer foundation for Orchbee. The current slice owns one
explicit loopback or trusted-LAN-preview listener, Personal Access Token exchange, signed Browser
session Cookie admission, WebSocket Upgrade integration on that same listener, trusted Web
renderer delivery, and authenticated Browser Router fallback. It does not expose a general product
API or a production deployment contract.

The accepted future direction and reserved security decisions remain in
[`proposals/platform/connected-client-capability-service.md`](../../proposals/platform/connected-client-capability-service.md).

## Current boundary

Inputs are a trusted renderer distribution, a Personal Access Token verifier, a fresh session
signing key, and local process configuration. Outputs are HTTP responses, optional application
routes registered by an owning composition module, and a closeable listener handle. Neither input
secret appears in readiness output or Browser-visible state.

| Request class                                              | Current result                                                                               |
| ---------------------------------------------------------- | -------------------------------------------------------------------------------------------- |
| Existing `/static/*` renderer asset                        | Public asset response with long-lived immutable caching                                      |
| `GET` or `HEAD` `/login` accepting HTML                    | Public renderer document with revalidation required                                          |
| Same-origin `POST /api/v1/auth/session` with a valid token | `204` plus a signed `HttpOnly; SameSite=Strict; Path=/` session Cookie                       |
| Invalid or rate-limited session exchange                   | Bounded JSON error with `no-store`; no Cookie                                                |
| Authenticated unmatched `GET` or `HEAD` accepting HTML     | Renderer document with revalidation required, allowing the Browser Router to resolve the URL |
| Unauthenticated product document request                   | `302` to `/login` with one encoded relative return target; renderer HTML is not returned     |
| Missing `/static/*` renderer asset                         | `404`; never renderer HTML                                                                   |
| Unmatched `/api`, `/api/*`                                 | JSON `404`; never renderer HTML                                                              |
| Ordinary HTTP request to unmatched `/ws`, `/ws/*`          | JSON `426 Upgrade Required`; never renderer HTML                                             |
| Unknown WebSocket Upgrade                                  | Rejected; no generic or implicit WebSocket capability                                        |
| Other unmatched requests                                   | JSON `404`                                                                                   |

`/api/v1/*` and `/ws/v1/*` are the first reserved versioned application namespaces. Reserving
their parent prefixes prevents a misspelled or unsupported protocol path from becoming an HTML
navigation accidentally.

## Lifecycle and invariants

1. Process configuration is parsed before any socket is opened.
2. The renderer root, `index.html`, and `static/` directory must exist as trusted local build
   inputs before Fastify can become ready.
3. The listener defaults to `127.0.0.1`; only the explicit trusted-LAN preview value `0.0.0.0` is
   also accepted at configuration parsing.
4. Session exchange requires the exact request Origin, validates the token in constant time, and
   applies a bounded in-memory failure limit per source address.
5. The Cookie contains a random signed session identifier, never the Personal Access Token; a
   fresh signing key invalidates every prior session when Browser access restarts or the token is reset.
6. HTTP and WebSocket routes share one HTTP/1.1 listener and therefore one origin and port.
7. API, WebSocket, and static namespaces never fall through to the Browser Router entry.
8. Closing the returned running-server handle closes the shared listener and every connection it
   owns.

The renderer distribution is a deployment input, not an HTTP upload or user-selected directory.
External request data is never used to choose another filesystem root.

## Run locally

Build the Web renderer and Server, then provide the renderer root as an absolute path:

```sh
pnpm --filter @orchbee/web build
pnpm --filter @orchbee/server build
ORCHBEE_SERVER_RENDERER_ROOT=/absolute/path/to/apps/web/dist \
ORCHBEE_SERVER_PERSONAL_ACCESS_TOKEN_VERIFIER=<43-character-base64url-verifier> \
ORCHBEE_SERVER_SESSION_SIGNING_KEY=<43-character-base64url-key> \
  pnpm --filter @orchbee/server start
```

`ORCHBEE_SERVER_PORT` defaults to `0`, so the operating system selects an available port. The
process emits one structured, non-secret readiness record containing the actual host and port.
`ORCHBEE_SERVER_HOST` may be omitted, set to `127.0.0.1`, or explicitly set to `0.0.0.0`. Desktop
uses the last value only after the user chooses trusted-LAN preview and pairs this static server
with an exact-Origin, Cookie-authenticated Gateway. Desktop generates and transfers both required
authentication inputs; they are not operator-facing deployment credentials.

## Deliberate omissions

The `0.0.0.0` mode is a trusted-LAN HTTP preview, not a production deployment mode. This Server
keeps static assets and the login document public so an unauthenticated page can render the sign-in
flow. It does not claim TLS, Internet exposure, discovery, multi-user accounts, token recovery,
durable rate-limit coordination, background-service operation, general product APIs, or production
WebSocket hosting. The same-origin token exchange and exact-Origin Gateway checks reduce Browser
request forgery; they do not encrypt HTTP traffic on a local network.
