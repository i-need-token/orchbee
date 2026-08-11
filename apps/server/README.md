# `@orchbee/server`

`@orchbee/server` is the runnable HTTP renderer foundation for Orchbee. The current slice owns one
explicit loopback or trusted-LAN-preview listener, WebSocket Upgrade integration on that same
listener, trusted Web renderer delivery, and Browser Router fallback. It does not expose a product
API or a production WebSocket contract.

The accepted future direction and reserved security decisions remain in
[`proposals/platform/connected-client-capability-service.md`](../../proposals/platform/connected-client-capability-service.md).

## Current boundary

Inputs are a trusted renderer distribution and local process configuration. Outputs are HTTP
responses, optional application routes registered by an owning composition module, and a
closeable listener handle.

| Request class                                     | Current result                                                                                   |
| ------------------------------------------------- | ------------------------------------------------------------------------------------------------ |
| Existing `/static/*` renderer asset               | Asset response with long-lived immutable caching                                                 |
| Missing `/static/*` renderer asset                | `404`; never renderer HTML                                                                       |
| Unmatched `/api`, `/api/*`                        | JSON `404`; never renderer HTML                                                                  |
| Ordinary HTTP request to unmatched `/ws`, `/ws/*` | JSON `426 Upgrade Required`; never renderer HTML                                                 |
| Unknown WebSocket Upgrade                         | Rejected; no generic or implicit WebSocket capability                                            |
| Other unmatched `GET` or `HEAD` accepting HTML    | Renderer `index.html` with revalidation required, allowing the Browser Router to resolve the URL |
| Other unmatched requests                          | JSON `404`                                                                                       |

`/api/v1/*` and `/ws/v1/*` are the first reserved versioned application namespaces. Reserving
their parent prefixes prevents a misspelled or unsupported protocol path from becoming an HTML
navigation accidentally.

## Lifecycle and invariants

1. Process configuration is parsed before any socket is opened.
2. The renderer root, `index.html`, and `static/` directory must exist as trusted local build
   inputs before Fastify can become ready.
3. The listener defaults to `127.0.0.1`; only the explicit trusted-LAN preview value `0.0.0.0` is
   also accepted at configuration parsing.
4. HTTP and WebSocket routes share one HTTP/1.1 listener and therefore one origin and port.
5. API, WebSocket, and static namespaces never fall through to the Browser Router entry.
6. Closing the returned running-server handle closes the shared listener and every connection it
   owns.

The renderer distribution is a deployment input, not an HTTP upload or user-selected directory.
External request data is never used to choose another filesystem root.

## Run locally

Build the Web renderer and Server, then provide the renderer root as an absolute path:

```sh
pnpm --filter @orchbee/web build
pnpm --filter @orchbee/server build
ORCHBEE_SERVER_RENDERER_ROOT=/absolute/path/to/apps/web/dist \
  pnpm --filter @orchbee/server start
```

`ORCHBEE_SERVER_PORT` defaults to `0`, so the operating system selects an available port. The
process emits one structured, non-secret readiness record containing the actual host and port.
`ORCHBEE_SERVER_HOST` may be omitted, set to `127.0.0.1`, or explicitly set to `0.0.0.0`. Desktop
uses the last value only after the user chooses trusted-LAN preview and pairs this static server
with an exact-Origin, capability-authenticated Gateway.

## Deliberate omissions

The `0.0.0.0` mode is a trusted-LAN HTTP preview, not a production deployment mode. This Server
does not authenticate static renderer requests; terminal authority remains behind the separate
Gateway's exact-Origin and process-lifetime capability checks. Token-to-Cookie login, Cookie scope,
TLS, Internet exposure, discovery, CSRF protection, rate limits, background service operation,
actual API routes, and actual WebSocket messages remain omitted. The future
`remote-client-protocol` and `capability-node-protocol` packages are therefore not created here.
