# `@orchbee/server`

`@orchbee/server` is the runnable foundation for the Orchbee application backend. The current
slice owns one loopback HTTP listener, WebSocket Upgrade integration on that same listener,
trusted Web renderer delivery, and Browser Router fallback. It does not yet expose a product API
or a production WebSocket contract.

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
3. The listener binds only to `127.0.0.1`; other hosts are rejected at configuration parsing.
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

`ORCHBEE_SERVER_PORT` defaults to `0`, so the operating system selects an available port and the
process prints the resulting short origin. `ORCHBEE_SERVER_HOST` may be omitted or explicitly set
to `127.0.0.1`; no other value is accepted in this slice.

## Deliberate omissions

This foundation does not authorize LAN binding. Token-to-Cookie login, Cookie scope, TLS, Origin
policy, CSRF protection, rate limits, sharing activation, Desktop child-process supervision,
actual API routes, and actual WebSocket messages require separately accepted slices. The future
`remote-client-protocol` and `capability-node-protocol` packages are therefore not created here.
