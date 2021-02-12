# Another Rust Load Balancer

<p align="center">
<img src="assets/logo_400x400.png" alt="ARLB" title="ARLB" />
</p>

ARLB (Another Rust Load Balancer) is a reverse proxy and load balancer written in Rust based on `hyper` and `tokio`.

## Features

- HTTP & HTTPS Termination
- HTTP1.1 & HTTP2
- IPv4 & IPv6 Listeners
- Load Balancing Strategies
  - IP Hash
  - Least Connection
  - Random
  - Round Robin
  - Sticky Cookie
- Middlewares
  - Compression (gzip, deflate, brotli)
  - HTTP Basic Auth (LDAP)
  - HTTP to HTTPS Redirect
  - Custom Error Pages
  - Max Request Body Size
  - Rate Limiting
- Health Checks
- ACME
- Advanced Backend Matching Strategies
- File based configuration
- Reload configuration without restarting the process
- Fast
- Secure

## Getting Started

Please have a look at the [Getting Started](docs/getting_started.md) guide.

## Documentation

- [Architecture](docs/architecture.md)
- [Configuration](docs/configuration.md)
- [Load Balancing Strategies](docs/lb_strategies.md)
- [Middlewares](docs/middlewares.md)
- [Backend Pool Matching](docs/backend_matching.md)
- [Health Checks](docs/health_checks.md)
- [Certificates/ACME](docs/certificates.md)

## Authors/Contributors

This project was created for the `High level languages: Rust` course (winter term 20/21) of LMU Munich.

- Adrodoc
- Zynaa
- Jonas Dellinger
- lor-enz
- Martinif
- skess42
