# Load Balancing Strategies

Once a request has been matched to a backend pool, the load balancing strategy decides which backend will receive the request.

## IP Hash

Hashes the IP of the underlying socket of the request.

```toml
strategy = { IPHash = {} }
```

## Least Connection

Keeps track of all open connections to backend servers and chooses the one with the least open connections. If two or more have the same amount, a random one will be drawn of them.

```toml
strategy = { LeastConnection = {} }
```

> ⚠ A connection pool is used by default, so connections will be held open. This could distort the load balancing when least connection is used. Have a look at the [configuration](configuration.md) if you want to disable connection pooling.

# Random

Selects a random address

```toml
strategy = { Random = {} }
```

## Round Robin

Cycles through each address by keeping an internal counter.

```toml
strategy = { RoundRobin = {} }
```

## StickyCookie

On the first request, the `inner` strategy is used to select the backend server. The response will include a cookie, which contains the address of the previously selected backend server. Subsequent requests which contain the cookie will be forwarded to the address inside the cookie.

Information for the cookie configuration (`http_only` etc.) can be found at [Set-Cookie](https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Set-Cookie#attributes)

```toml
strategy = { StickyCookie = { cookie_name = "lb_cookie", http_only = false, secure = false, same_site = { Lax = {} }, inner = { RoundRobin = {} } } }
```
