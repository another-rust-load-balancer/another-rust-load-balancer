# Health checks

Health checks perform requests to the backend servers on a regular interval. The response itself, the time until it arrives or the lack of a response are all used to determine the Healthiness of each server as either healthy, slow or unresponsive.

## Health classes

Depending on the health values of the servers the forwarding of client requests changes.

- **Healthy** servers are used for client requests.
- **Slow** servers are only used for client requests when no healthy servers are available.
- **Unresponsive** servers are not used.

## Identifying unhealthy servers:

On a defined time interval ARLB sends http requests to all backend servers.

- Servers are declared **unresponsive** if the request results in a timeout or the status code is not in the success class of status codes (200 - 299).
- Servers are declared **slow** if the response takes longer than a given threshold.

Once unresponsive servers pass another health check they become available again for handling client requests.


## Defining parameteres in TOML

Custom values in the configuration TOML file can be used to alter the behaviour of the health checks. This usage of this is optional.

- `path` sets the path component of the request address. The default value is `/`.
- `slow_threshold` sets the response time (in ms) above which a server is categorized as slow. The default value is `300` ms.
- `timeout` Specifies the time (in ms) after which the health check is aborted and the server declared unresponsive. The default value is `500` ms.

### Examples:

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

A time interval for the health checks is set globally for all backend pools. The number represents seconds. The default value is 10 seconds.

```
[health_interval]
check_every = 5
```

