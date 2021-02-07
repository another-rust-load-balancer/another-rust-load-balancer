# Configuration

The configuration is supplied via a local TOML file

```toml
# backend.toml

[[backend_pools]]
matcher = "Host('whoami.localhost')"
addresses = ["127.0.0.1:8080", "127.0.0.1:8081", "127.0.0.1:8082"]
schemes = ["HTTP", "HTTPS"]
strategy = { RoundRobin = {} }
[backend_pools.middlewares]
HttpsRedirector = {}

[[backend_pools]]
matcher = "Host('youtube.de') && Path('/admin')"
addresses = ["192.168.0.2:3000"]
schemes = ["HTTPS"]
strategy = { RoundRobin = {} }
[backend_pools.middlewares]
Compression = {}

[certificates]
"whoami.localhost" = { certificate_path = "x509/whoami.localhost.cer", private_key_path = "x509/whoami.localhost.key" }
"youtube.de" = { certificate_path = "x509/youtube.de.cer", private_key_path = "x509/youtube.de.key" }
```

It currently contains two top level entries:

- A list of `backend_pools`
- A dictionary/map of `certificates`

## `[[backend_pools]]`

A backend pool is used to specify how matching incoming requests should be modified and to which location they should be forwarded to. Each backend pool needs to specify the following **required** keys:

- `matcher`
- `addresses`
- `schemes`
- `strategy`

The following keys are optional:

- `middlewares`
- `client`

### `matcher`

Defines when a request should be matched to this backend pool.

Examples:

```toml
# Standard host header matching
matcher = "Host('whoami.localhost')"

# matches whoami.localhost and all subdomains *.whoami.localhost
matcher = "HostRegexp('(.*\\.)?whoami\\.localhost$')"

# A very open matcher
matcher = "Path('/')"

# Always matches
matcher = "HostRegexp('*')"

# && and || are supported
matcher = "Host('whoami.localhost') && Path('/')"

# nested && and || need brackets
matcher = "Host('whoami.localhost') && (Path('/') || Path('/admin'))"
```

A full list of supported expressions can be found in [Backend Matching](backend_matching.md)

### `addresses`

A list of backend addresses for this pool. Can be supplied in IPv4 or IPv6 syntax.

Examples:

```toml
addresses = ["[::1]:8084", "127.0.0.1:8085", "[2001:3200:3200::1:6]:80"]

addresses = ["172.28.1.1:80", "172.28.1.2:80", "172.28.1.3:80"]

# local and single addresses are also supported
addresses = ["127.0.0.1:3000"]
```

### `schemes`

A list of supported schemes, only `HTTP` and `HTTPS` are supported.

Examples:

```toml
schemes = ["HTTP"]

schemes = ["HTTP", "HTTPS"]

# Will print a warning and makes your backend pool unreachable
schemes = []
```

### `strategy`

A load balacing strategy and its configuration.

Examples:

```toml
strategy = { RoundRobin = {} }

strategy = { IPHash = {} }

strategy = { StickyCookie = { cookie_name = "lb_cookie", http_only = false, secure = false, same_site = { Lax = {} }, inner = { RoundRobin = {} } } }
```

A full list of supported strategies and their configuration can be found in [Load Balancing Strategies](lb_strategies.md)

### `middlewares`

A map/dictionary of middlewares to apply to the request/response.

Examples:

```toml
# Order of middlewares is kept
[backend_pools.middlewares]
HttpsRedirector = {}
Compression = {}
```

A full list of middlewares and their configuration can be found in [Middlewares](middlewares.md)

### `client`

An object which can be used to configure the `hyper` client, which is used to make requests to the backend.

Examples:

```toml
# Sets the maximum idle connection per host allowed in the pool.
# 0 disables to connection pool
client = { pool_max_idle_per_host = 0 }

# Set an optional timeout for idle sockets being kept-alive.
client = { pool_idle_timeout = { secs = 5, nanos = 0 } }
```

## `[certificates]`

// TODO: ACME?

A map/dictionary of local certificates.

Examples:

```toml
[certificates]
"whoami.localhost" = { certificate_path = "x509/whoami.localhost.cer", private_key_path = "x509/whoami.localhost.key" }
"youtube.de" = { certificate_path = "x509/youtube.de.cer", private_key_path = "x509/youtube.de.key" }
```
