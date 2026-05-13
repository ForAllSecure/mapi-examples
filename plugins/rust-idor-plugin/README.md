This rust plugin detects [IDOR / BOLA](https://owasp.org/API-Security/editions/2023/en/0xa1-broken-object-level-authorization/)
(Insecure Direct Object Reference / Broken Object Level Authorization) by
re-issuing each request with alternate user credentials and comparing the
responses.

It implements **both** the rewrite and classify gRPC services in a single
process, listening on two separate ports so the rewrite and classify
traffic can be observed and debugged independently. mAPI is pointed at each
service via its own port.


# How it works

mAPI's plugin system invokes `Rewrite` on a request before it's sent and
`Classify` on the response after it returns. The two calls are sequential
per-request but concurrent across requests, and the proto does not carry a
correlation ID. This plugin works around that by stamping a unique URL
fragment (`#mapi-idor-<uuid>`) onto each request — fragments are preserved in
the `request_url` field passed to `Classify` but stripped before the wire
request, so the target never sees them.

Each request first passes through three cheap filters in `Rewrite`. If any
filter trips, the plugin returns the request untouched (no fragment, no
side-channels, no stash):

* **Method filter** — only methods listed in `MAPI_IDOR_METHODS` (default
  `GET,PUT,DELETE`) are checked. POST is excluded by default because creates
  typically return identity-independent responses (`{"status": "added"}`)
  and produce noise.
* **Path-shape filter** — IDOR is object-level authorization, so the path
  needs at least 2 non-empty segments (e.g. `/collection/{id}`). Single-
  segment paths like `/health`, `/locations`, `/info` aren't IDOR-eligible.
* **Blocklist** — `(method, shape)` tuples that have already been flagged
  or identified as publicly accessible. Shape canonicalization replaces the
  last segment with `*` (`/driver/foo` → `/driver/*`), so one finding on
  `/driver/admin` dedups all sibling `/driver/<anything>` hits.

For requests that pass all three, the plugin:

1. Fires **N+1 side-channel requests in parallel**: one per configured alt
   identity, plus one "canary" with the auth header removed entirely.
2. Stashes the side-channel results under the correlation UUID.
3. Returns the rewritten request to mAPI with the fragment appended; mAPI
   sends its own version to the target with the primary user's credentials.
4. On `Classify`, looks up the stashed results by fragment, and applies a
   canary gate before per-alt comparison:
   - **Canary 2xx + body ≈ original** → endpoint is publicly accessible.
     Suppress + blocklist the shape.
   - **Canary non-2xx + body ≈ original** → the original's "2xx" is
     effectively an error response in disguise (same shape as the no-auth
     denial). Suppress this hit, but don't blocklist — a valid ID on the
     same shape may return real data later.
   - **Otherwise** → endpoint is auth-gated and original is data-bearing.
     For each alt identity whose response matches the original's status
     code AND has a body of similar length, emit an `IDOR/BOLA` issue
     with a **canonical, shape-based summary** (e.g. `... on GET /driver/*`).
     The plugin deliberately emits on every matching hit; mAPI's own dedup
     collapses identical summaries to a single finding per shape.
     Concrete URL and body-match percentage are logged to stderr only.

Body similarity is computed as `min(len_a, len_b) / max(len_a, len_b)`,
with a configurable threshold (default `0.8`). Findings are sampled by
default at 1 in 10 requests to keep the load multiplier reasonable.


# Compiling

## Prerequisites

* [rust 1.70 or higher](https://www.rust-lang.org/)
* [`tonic` dependencies](https://github.com/hyperium/tonic#dependencies)
* [`mapi` CLI](https://mayhem4api.forallsecure.com/docs/ch01-01-installation.html)


## Build

In the directory of this readme, run:

```shell
cargo build --release
```

The binary will be in `./target/release/rust-idor-plugin`.


# Configuration

## Alternate identities file

The plugin needs a JSON file describing one or more **alternate identities**
to compare against. Each entry specifies the header to swap and the value to
use:

```json
[
  {
    "label": "user_b",
    "header": "Authorization",
    "value": "Bearer eyJ..."
  },
  {
    "label": "expired",
    "header": "Authorization",
    "value": "Bearer expired..."
  }
]
```

Notes:

* `label` is what appears in the issue summary — pick something readable.
* `header` can be anything (`Authorization`, `X-API-Key`, etc.). Different
  identities can use different headers.
* The plugin **automatically** also fires an unauthenticated canary probe
  (auth header stripped). You do **not** need to add an `"anonymous"` entry.
* The file must contain at least one identity.

Path is provided via `MAPI_IDOR_ALT_IDENTITIES`.


## Environment variables / CLI flags

| Variable / flag                          | Default            | Description |
|------------------------------------------|--------------------|-------------|
| `MAPI_IDOR_REWRITE_PORT`                 | `50051`            | gRPC port for the rewrite service. |
| `MAPI_IDOR_CLASSIFY_PORT`                | `50052`            | gRPC port for the classify service. |
| `MAPI_IDOR_ALT_IDENTITIES`               | *(required)*       | Path to the identities JSON file. |
| `MAPI_IDOR_SAMPLE_RATE`                  | `10`               | Run the IDOR check on 1 in N requests. Set to `1` for every request. |
| `MAPI_IDOR_CANARY_HEADER`                | `Authorization`    | Header name to strip on the canary probe. |
| `MAPI_IDOR_METHODS`                      | `GET,PUT,DELETE`   | Comma-separated HTTP methods to IDOR-check. Add `POST` to also test creates. |
| `MAPI_IDOR_BODY_SIMILARITY_THRESHOLD`    | `0.8`              | Min `min(len)/max(len)` body-length ratio required to flag. `1.0` = exact length, `0.0` = ignore body, status-only. Also used as the canary-vs-original gate. |
| `MAPI_IDOR_SIDE_CHANNEL_TIMEOUT_MS`      | `10000`            | Per-side-channel HTTP timeout. |
| `MAPI_IDOR_STATE_TTL_SECS`               | `60`               | TTL on stashed side-channel results (orphan eviction). |
| `MAPI_IDOR_INSECURE_TLS`                 | `false`            | Disable TLS verification for side-channel requests. **Dangerous** — see Caveats. |


# Usage

## Starting the plugin

```shell
MAPI_IDOR_ALT_IDENTITIES=./identities.json \
MAPI_IDOR_SAMPLE_RATE=1 \
  ./target/release/rust-idor-plugin
```

You should see something like:

```
loaded 1 alternate identities; sample rate 1-in-1; canary strips 'Authorization'
Listening: rewrite on 0.0.0.0:50051, classify on 0.0.0.0:50052
```


## Running mAPI against the plugin

Point `--rewrite-plugin` at the rewrite port and `--classify-plugin` at the
classify port:

```shell
mapi run workspace/target/project \
  60 \
  path/to/openapi.json \
  --url https://your-target/ \
  --header-auth "<auth>" \
  --rewrite-plugin http://localhost:50051 \
  --classify-plugin http://localhost:50052
```


## Expected output

When the plugin finds IDOR, mAPI surfaces it as a Custom Issue. The summary
is intentionally prefixed `IDOR/BOLA:` so it's easy to search for in the UI,
and uses the shape-canonical path (`/*` for the variable segment) so all
concrete-URL hits collapse to one finding per shape.


# Caveats

* **Load multiplier.** For each IDOR-checked request, the plugin sends N+1
  extra requests to the target (N alt identities + 1 canary). Sampling at
  1-in-10 keeps the average around `1 + (N+1)/10` requests per fuzz request.
  For demos, set `MAPI_IDOR_SAMPLE_RATE=1` to see findings faster; for
  production runs, the default is more polite.
* **Body comparison is length-only.** Two responses of similar size but
  totally different content will appear similar. 
* **Spec is not consulted.** The plugin makes no assumption that an endpoint
  is auth-protected — it discovers that empirically via the canary. This
  means real IDOR is detected even on endpoints whose spec is wrong or
  missing a `security` block.
* **Path-shape canonicalization is heuristic.** The summary canonicalizes
  only the last path segment with `*`, so `/users/{id}/profile` and
  `/users/{id}/orders` would each get their own finding rather than
  collapsing into a single `/users/{id}/*` entry.
* **Single-segment endpoints are never checked.** `/health`, `/me`,
  `/info` and similar paths are skipped regardless of whether they
  return per-user data. The trade-off: per-user resources without an `{id}`
  in the path (e.g. `/me`) won't be tested, but neither will global
  endpoints that are shared by design.
* **TLS verification is on by default.** Side-channel requests verify the
  target's certificate just like a normal HTTP client. To run against
  self-signed or expired certs (common in local/staging environments), set
  `MAPI_IDOR_INSECURE_TLS=true` — but understand the risk: side-channels
  carry the alt identities (real credentials) and the original request
  body, so an attacker on the network path can MITM and harvest them. Use
  only against trusted local targets.
