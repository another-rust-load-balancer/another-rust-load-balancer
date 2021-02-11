## Matching Backends

Every backend pool requires a `matcher` field. This field is used to decide if incoming requests should be forwarded to the respective backend pool. If multiple backend pools are configured, the matcher of each pool will be called in the order they appear in the config until one match was successful. If no match was successful, a `404 Not Found` is returned.

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

# nested && and || need brackets
matcher = "(Host('whoami.localhost') || Host('whoami.de')) && (Path('/') || Path('/admin'))"
```

Here is a list of all supported matchers:

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
