# Another Rust Load Balancer

## HTTPS

To use HTTPS you need certificates and private keys in the directory `x509`.\
You can generate these for `localhost` and `www.arlb.de` along with a certificate authority by running `./generate-ca-and-server-certificates.sh`.

For the server to be reachable via `www.arlb.de` you can add the following to the `etc/hosts` file:

```
127.0.0.1 www.arlb.de
```

For browsers to trust these generated certificates you have to import the generated certificate authority file `x509/ca.cer`.

## Matching Backends

Every backend pool requires a `matcher` field. This field is responsible for checking if incoming requests should be forwarded to the respective backend pool. If multiple backend pools are configured, the matcher of each pool will be called in the order they're declared in the config until one match was successful. A matcher field can consist of the following rules:

### Host

Passes requests when the request's `Host` header matches the supplied string.

<details>
<summary>Example</summary>
<br>

```toml
[[backend_pools]]
matcher="Host('google.localhost')"
```

- ✔ `google.localhost`
- ✔ `google.localhost/test`
- ✔ `google.localhost/path?query=false`
- ❌ `test.google.localhost`
- ❌ `google.de`

</details>

---

### HostRegexp

Passes requests when the request's `Host` header matches the supplied regex.

<details>
<summary>Example</summary>
<br>

```toml
[[backend_pools]]
matcher="HostRegexp('(.*\\.)?whoami\\.localhost$')"
```

- ✔ `whoami.localhost`
- ✔ `test.whoami.localhost/test`
- ✔ `test.nested.whoami.localhost`
- ❌ `whoami.localhostwhat`
- ❌ `test.whoami.localhostwhat`

</details>

---

### Method

Passes requests when the request's method matches the supplied method.

<details>
<summary>Example</summary>
<br>

```toml
[[backend_pools]]
matcher="Method('GET')"
```

- ✔ GET `whoami.localhost`
- ❌ POST `test.whoami.localhost/test`
- ❌ YOLO `test.whoami.localhostwhat`

</details>

---

### Path

Passes requests when the request's URI's path matches the supplied path string.

<details>
<summary>Example</summary>
<br>

```toml
[[backend_pools]]
matcher="Path('/admin')"
```

- ✔ `whoami.localhost/admin`
- ❌ `whoami.localhost`
- ❌ `whoami.localhost/admin/test`

</details>

---

### PathRegexp

Passes requests when the request's URI's path matches the supplied path regex.

<details>
<summary>Example</summary>
<br>

```toml
[[backend_pools]]
matcher="Path('^/admin/.*')"
```

- ✔ `whoami.localhost/admin`
- ✔ `whoami.localhost/admin/nested`
- ❌ `whoami.localhost`

</details>

---

### Query

Passes requests when the request's query contains a specific `key` with a `value`

<details>
<summary>Example</summary>
<br>

```toml
[[backend_pools]]
matcher="Query('admin', 'true')"
```

- ✔ `whoami.localhost?admin=true`
- ❌ `whoami.localhost?admin=false`
- ❌ `whoami.localhost`

</details>

---

### && (AND)

Passes requests when the `left` and `right` side evaluate to `true`

Nested `&&` or `||` expressions must be enclosed with `( )`

<details>
<summary>Example</summary>
<br>

```toml
[[backend_pools]]
matcher="Host('google.de') && Path('/admin')"
```

- ✔ `google.de/admin`
- ❌ `google.de`
- ❌ `whoami.localhost`
- ❌ `whoami.localhost/admin`

</details>

---

### || (OR)

Passes requests when the `left` or `right` side evaluate to `true`

Nested `&&` or `||` expressions must be enclosed with `( )`

<details>
<summary>Example</summary>
<br>

```toml
[[backend_pools]]
matcher="Host('google.de') || Host('google.com')"
```

- ✔ `google.de`
- ✔ `google.com`
- ✔ `google.com/admin`
- ❌ `google.io`

</details>

---
