# Certificates

ARLB supports two different kind of certificates:

- `Local`
  > A local certificate on the file system is used. This requires a .cer and .key file
- `ACME`
  > An ACME (Automatic Certificate Management Environment) is used to automatically generate certificates for your domain.

## Local

A local certificate is used to secure the HTTPS connection. A path to a `.cer` and `.key` file needs to be provided..

```toml
[certificates]
"whoami.localhost" = { Local = { certificate_path = "../certificates/whoami.localhost.cer", private_key_path = "../certificates/whoami.localhost.key" } }
"youtube.de" = { Local = { certificate_path = "../certificates/youtube.de.cer", private_key_path = "../certificates/youtube.de.key" } }
```

In the `/examples/certificates` folder two scripts can be used to generated local self-signed cerificates. The output will be located in the current working directory. If you want to test the related configs, make sure the generated certificates are located inside `/examples/certificates`.

- `generate-ca-certificate.sh`: Generates a CA certificate and private key. This only needs to be run once.
- `generate-server-certificate.sh [domain]`: Generates a certificate and a private key for the given `domain`. Example call: `generate-server-certificate.sh https.localhost`

## ACME

> To enable HTTPS on your website, you need to get a certificate (a type of file) from a Certificate Authority (CA). Let’s Encrypt is a CA. In order to get a certificate for your website’s domain from Let’s Encrypt, you have to demonstrate control over the domain. With Let’s Encrypt, you do this using software that uses the ACME protocol which typically runs on your web host.

Instead of using self-signed or local certificates, an ACME certificate ensures that it's signed by a valid CA and is automatically renewed once it's close to being expired. In the process of getting an ACME certificate, the ACME server will have to verify ownership of your specified domain. **So make sure your domain is pointing to the IP of your ARLB instance**.

In ARLB, the ACME server is always `Let's Encrypt`. If you don't need a production certificate (creating production certificates are rate limited), you can generate a staging certificate by set the `staging` flat to `true`.

```toml
[certificates]
"staging.youtube.de" = { ACME = { email = "yourmail@example.com", persist_dir = "./certificates", staging = true } }
"youtube.de" = { ACME = { email = "yourmail@example.com", persist_dir = "./certificates", staging = false } }
```
