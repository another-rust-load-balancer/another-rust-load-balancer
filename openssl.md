Steps to create our CA and sign the first server certificate as described in <https://www.makethenmakeinstall.com/2014/05/ssl-client-authentication-step-by-step/>.

```bash
openssl req -subj "/C=DE/L=Muenchen/O=Another Rust Load Balancer/CN=root" -newkey rsa:4096 -keyform PEM -keyout ca.key -x509 -days 3650 -outform PEM -out ca.cer -nodes
openssl genrsa -out server.key 4096
openssl req -new -subj "/C=DE/L=Muenchen/O=Another Rust Load Balancer/CN=localhost" -addext "subjectAltName = DNS:localhost" -key server.key -out server.req -sha256
openssl x509 -req -in server.req -CA ca.cer -CAkey ca.key -set_serial 1 -extensions server -days 1024 -outform PEM -out server-1.cer -sha256
cat ca.cer >> server-1.cer
```
