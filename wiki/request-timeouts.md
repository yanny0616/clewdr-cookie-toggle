# Request timeouts and logs

Clewdr applies a 270-second deadline to each Claude message request, including
credential selection/refresh, retries and the response body (streaming or not).
Set `CLEWDR_REQUEST_TIMEOUT_SECONDS` to a positive integer to change it; restart
the service after changing this environment variable.

Set the reverse proxy and caller timeouts above Clewdr's deadline. For example,
inside the OpenResty/Nginx location that proxies Clewdr:

```nginx
proxy_connect_timeout 30s;
proxy_send_timeout 300s;
proxy_read_timeout 300s;
proxy_buffering off;
```

Check with `nginx -t`, then reload Nginx. A bind-mounted proxy configuration
change does not require rebuilding or pulling the proxy image.

Before response headers, Clewdr returns HTTP 504 with error type
`request_timeout`. After streaming headers have been sent, HTTP status cannot be
changed; the body closes with an error and the request log records the timeout.
These deadlines bound waiting; they do not make the upstream model generate faster.

One log row represents an incoming message request, including its retries.
Statuses: `pending`, `success`, `error`, `timeout`, `cancelled`, `interrupted`.
Client/proxy disconnection cancels the row; Clewdr cannot distinguish the remote
party's timeout from other cancellations. On startup, persisted pending rows
become interrupted with unknown duration/cause. No upstream error is invented
for old rows. Token usage updates cannot overwrite a terminal failure.

The log UI refreshes every five seconds, shows live elapsed time for pending
requests and supports status/search filters and expandable error/usage details.

Regression checks:

```sh
CLEWDR_NO_FS=true cargo test --lib --no-default-features --features external-resource,xdg request_log -- --test-threads=1
cargo check -p clewdr-frontend --target wasm32-unknown-unknown
```
