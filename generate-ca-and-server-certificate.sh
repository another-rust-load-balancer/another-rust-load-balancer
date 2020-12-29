#!/bin/bash

# Create our CA and sign a server certificate as described in https://www.makethenmakeinstall.com/2014/05/ssl-client-authentication-step-by-step/ and https://security.stackexchange.com/a/176084.

mkdir x509
cd x509
openssl req -subj "/C=DE/ST=Deutschland/L=Muenchen/O=Another Rust Load Balancer/CN=root" -newkey rsa:4096 -keyform PEM -keyout ca.key -x509 -days 3650 -outform PEM -out ca.cer -nodes
openssl genrsa -out server.key 4096
openssl req -new -subj "/C=DE/ST=Deutschland/L=Muenchen/O=Another Rust Load Balancer/CN=localhost" -addext "subjectAltName=DNS:localhost" -key server.key -out server.csr -sha256
echo "[server]
subjectAltName=DNS:localhost
" > ssl-extensions-x509.cnf
openssl x509 -req -in server.csr -CA ca.cer -CAkey ca.key -set_serial 1 -extensions server -days 1024 -outform PEM -out server.cer -sha256 -extfile ssl-extensions-x509.cnf
cat ca.cer >> server.cer
