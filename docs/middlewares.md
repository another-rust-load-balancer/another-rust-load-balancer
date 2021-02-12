# Middlewares

## Authentication

This middleware allows securing a backend pool with HTTP Basic Auth using an LDAP server for user management.

Parameters:

- `ldap_address`: The address of the LDAP server.
- `user_directory`: Directory where the users are stored.
- `rdn_identifier`: The attribute that should be used as a username.
- `recursive`: Indicates whether subdirectories of `user_directory` should be searched.

```toml
[backend_pools.middlewares.Authentication]
ldap_address = "ldap://172.28.1.7:1389"
user_directory = "dc=example,dc=org"
rdn_identifier = "cn"
recursive = true
```

## Compression

If the client supports compression (`Accept-Encoding` header), the response from the backend server will be compressed.

Supported algorithms:

- gzip
- deflate
- brotli

```toml
[backend_pools.middlewares.Compression]
```

## Custom Error Pages

If the backend server responds with a matching status code, a HTML file named `{STATUS_CODE}.html` inside the provided `location` folder will be sent to the client instead. The `location` will be relative from the current working directory, **not the configuration file location**.

```toml
[backend_pools.middlewares.CustomErrorPages]
location = "../../errorpages"
errors = [404, 500]
```

## HTTPS Redirector

All requests sent via HTTP will receive a `301 Moved Permanently` and will be redirected to the `HTTPS` version of the URL.

```toml
[backend_pools.middlewares.HttpsRedirector]
```

## Max Body Size

All requests with a body size, specified in the `Content-Length` request header, greater than the provided threshold will be aborted and a response of `413 Payload Too Large` is returned.

```toml
[backend_pools.middlewares.MaxBodySize]
limit = 256
```

## Rate Limiter

If a client sends more than `limit` messages within `window_sec` seconds, they will be rejected with a `429 Too Many Requests` response.

```toml
[backend_pools.middlewares.RateLimiter]
limit = 2
window_sec = 10
```
