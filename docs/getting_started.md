# Getting Started

## Install

### Build yourself

Requirements:

- git
- Rust Toolchain

To build the project yourself, clone the project and build it using `cargo`:

```sh
git clone https://github.com/another-rust-load-balancer/another-rust-load-balancer.git
cd another-rust-load-balancer

cargo build # or `cargo build --release` for better performance
sudo setcap 'cap_net_bind_service=+ep' target/debug/another-rust-load-balancer # Allow to bind to port 80/443 without root

target/debug/another-rust-load-balancer --version
```

If you want to cross compile the project, we recommend using `cross`:

```sh
cargo install --version 0.1.16 cross # 0.1.16 still contains openssl

rustup target add x86_64-unknown-linux-musl
cross build --target x86_64-unknown-linux-musl --release
target/x86_64-unknown-linux-musl/release/another-rust-load-balancer --version
```

After that, you may want to add the binary to your `PATH`.

### Grab the binary

There may be a pre-compiled binary on the [github release page](https://github.com/another-rust-load-balancer/another-rust-load-balancer/releases)

## Running it

By default ARLB binds to port 80 (HTTP) and 443 (HTTPS). On linux, every port below 1024 is `privileged` so you either have to:

- run the program as root `sudo`
- use `setcap 'cap_net_bind_service=+ep' /path/to/another-rust-load-balancer`

Also, a minimal configuration file is required:

```toml
# backend.toml
[[backend_pools]]

matcher = "Host('whoami.localhost')"
addresses = ["127.0.0.1:8080", "127.0.0.1:8081", "127.0.0.1:8082"]
schemes = ["HTTP"]
strategy = { RoundRobin = {} }
```

This config file will load balance every request which is targeted to `whoami.localhost` between 3 local backend servers. Also, if the client supports compression, it will compress the backend server's response, thus saving bandwith.

> ℹ Don't have any backends for testing? You can use the following docker-compose file which will also match the above configuration. Put the contents in a `docker-compose.yml` and execute `docker-compose up -d`
>
> <details>
> <summary>docker-compose.yml</summary>
> <br>
>
> ```yml
> version: "3.7" # optional since v1.27.0
> services:
>   whoami01:
>     image: containous/whoami
>     ports:
>       - 8080:80
>   whoami02:
>     image: containous/whoami
>     ports:
>       - 8081:80
>   whoami03:
>     image: containous/whoami
>     ports:
>       - 8082:80
> ```
>
> </details>

Now we're ready to start the process:

```sh
# With setcap executed before
/path/to/another-rust-load-balancer --backend backend.toml

# With sudo
sudo /path/to/another-rust-load-balancer --backend backend.toml
```

You're now able to issue requests to `whoami.localhost` via your browser or favorite HTTP tool. When using the test `docker-compose` file, every server responds with its own hostname. Due to the round robing strategy, performing requests to `whoami.localhost` yields 3 different responses:

```sh
▶ curl -s -H "Host: whoami.localhost" 127.0.0.1 | grep "Hostname: "
Hostname: add142593362

▶ curl -s -H "Host: whoami.localhost" 127.0.0.1 | grep "Hostname: "
Hostname: ef8abdc45aef

▶ curl -s -H "Host: whoami.localhost" 127.0.0.1 | grep "Hostname: "
Hostname: 4c3e51c1ba8c
```

## Examples

More complex and advanced examples can be found in the `/examples` directory of the project. They showcase all configuration possibilities, load balancing strategies, middlewares and IPv6 usage.

```
# May need to start some of the docker-compose files in `/examples` first
# docker-compose up -d -f examples/docker-compose.yml

sudo /path/to/another-rust-load-balancer --backend examples/configs/https.toml
```
