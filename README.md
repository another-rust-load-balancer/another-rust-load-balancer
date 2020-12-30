# Another Rust Load Balancer

## HTTPS

To use HTTPS you need certificates and private keys in the directory `x509`.\
You can generate these for `localhost` and `www.arlb.de` along with a certificate authority by running `./generate-ca-and-server-certificates.sh`.

For the server to be reachable via `www.arlb.de` you can add the following to the `etc/hosts` file:

```
127.0.0.1 www.arlb.de
```

For browsers to trust these generated certificates you have to import the generated certificate authority file `x509/ca.cer`.
