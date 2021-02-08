# Middlewares

# Authentication

TODO

# Compression

If the client supports compression (`Accept-Encoding` header), the response from the backend server will be compressed.

Supported algorithms:

- gzip
- deflate
- brotli

```toml
[backend_pools.middlewares]
Compression = {}
```

# Custom Error Pages

If the backend server responds with a matching status code, a HTML file named `{STATUS_CODE}.html` inside the provided `location` folder will be sent to the client instead.

```toml
[backend_pools.middlewares]
CustomErrorPages = { location = "errorpages/", errors = [404, 500]}
```

# HTTPS Redirector

All requests sent via HTTP will receive a `301 Moved Permanently` and will be redirected to the `HTTPS` version of the URL.

```toml
[backend_pools.middlewares]
HttpsRedirector = {}
```

# Max Body Size

All requests with a `Content-Length` greater than the provided threshold will be aborted and a response of `413 Payload Too Large` is returned.

```toml
[backend_pools.middlewares]
MaxBodySize = 256
```
