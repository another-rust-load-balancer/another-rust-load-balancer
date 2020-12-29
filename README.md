# HTTPS
To use HTTPS you need a certificate and private key in the directory x509 named `server.cer` and `server.key` respectively.
You can generate these for `localhost` along with a certificate authority by running `./generate-ca-and-server-certificate.sh`.
For Browsers to trust these generated certificates you have to import the generated certificate authority file `x509/ca.cer`.
