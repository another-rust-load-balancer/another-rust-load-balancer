Steps to create our CA and sign the first server certificate as described in <https://www.makethenmakeinstall.com/2014/05/ssl-client-authentication-step-by-step/>.

```
another-rust-load-balancer/x509$ openssl req -newkey rsa:4096 -keyform PEM -keyout ca.key -x509 -days 3650 -outform PEM -out ca.cer -nodes
Generating a RSA private key
..++++
..............................................................................................................................................................................................................++++
writing new private key to 'ca.key'
-----
You are about to be asked to enter information that will be incorporated
into your certificate request.
What you are about to enter is what is called a Distinguished Name or a DN.
There are quite a few fields but you can leave some blank
For some fields there will be a default value,
If you enter '.', the field will be left blank.
-----
Country Name (2 letter code) [AU]:DE
State or Province Name (full name) [Some-State]:Deutschland
Locality Name (eg, city) []:Muenchen
Organization Name (eg, company) [Internet Widgits Pty Ltd]:Another Rust Load Balancer
Organizational Unit Name (eg, section) []:
Common Name (e.g. server FQDN or YOUR name) []:root
Email Address []:
```

```
another-rust-load-balancer/x509$ openssl genrsa -out server.key 4096
Generating RSA private key, 4096 bit long modulus (2 primes)
..................................................................................................................................................++++
......++++
e is 65537 (0x010001)
```

```
another-rust-load-balancer/x509$ openssl req -new -key server.key -out server.req -sha256
You are about to be asked to enter information that will be incorporated
into your certificate request.
What you are about to enter is what is called a Distinguished Name or a DN.
There are quite a few fields but you can leave some blank
For some fields there will be a default value,
If you enter '.', the field will be left blank.
-----
Country Name (2 letter code) [AU]:DE
State or Province Name (full name) [Some-State]:Deutschland
Locality Name (eg, city) []:Muenchen
Organization Name (eg, company) [Internet Widgits Pty Ltd]:Another Rust Load Balancer
Organizational Unit Name (eg, section) []:
Common Name (e.g. server FQDN or YOUR name) []:localhost
Email Address []:

Please enter the following 'extra' attributes
to be sent with your certificate request
A challenge password []:
An optional company name []:
```

```
another-rust-load-balancer/x509$ openssl x509 -req -in server.req -CA ca.cer -CAkey ca.key -set_serial 1 -extensions server -days 1024 -outform PEM -out server-1.cer -sha256
Signature ok
subject=C = DE, ST = Deutschland, L = Muenchen, O = Another Rust Load Balancer, CN = localhost
Getting CA Private Key
```
