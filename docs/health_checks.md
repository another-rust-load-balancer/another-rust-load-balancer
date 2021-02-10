# Health checks

Health checks perform request to the backend servers on a regular interval. The response, the response time or the lack of a response are all used to determine the Healthiness of each server as either Healthy, Slow or Unhealthy.

Depending on the health values of the servers the forwarding of client requests changes.

- **Healthy** servers are used for client requests.
- **Slow** servers are only used for client requests when no healthy servers are available.
- **Unresponsive** servers are not used.

Once unresponsive servers pass another Healthcheck they become available again for handling client requests.

Custom values in the configuration TOML file can be used to alter the behaviour of the health checks.

- `path` sets the uri of the server to check. The default value is `/`
- `slow_threshold` sets the response time above which a server is categorized as slow. The default value is `300`
- `timeout` Specifies the time after which the health check is aborted and the server declared unresponsive. The default value is `500`

Examples:

```
[backend_pools.health_config]
path = "/"
slow_threshold = 300
timeout = 500
```

```
[backend_pools.health_config]
path = "/health"
slow_threshold = 150
timeout = 300
```

A time interval for the health checks is set globally for all backend pools. Default is 10.

```
[health_interval]
check_every = 5
```
