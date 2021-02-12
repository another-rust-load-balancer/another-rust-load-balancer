#!/bin/bash

# Create our CA and sign a server certificate as described in https://www.makethenmakeinstall.com/2014/05/ssl-client-authentication-step-by-step/ and https://security.stackexchange.com/a/176084.

domain_name=$1

if [ -z "${domain_name}" ]; then
    echo "Usage: $0 <domain_name>" 1>&2
    exit 1
fi

typeset -i serial=$(cat serial.txt)+1
echo $serial > serial.txt

openssl genrsa -out $domain_name.key 4096
openssl req -new -subj "/C=DE/ST=Deutschland/L=Muenchen/O=Another Rust Load Balancer/CN=$domain_name" -addext "subjectAltName=DNS:$domain_name" -key $domain_name.key -out $domain_name.csr -sha256
echo "[server]
subjectAltName=DNS:$domain_name
" > $domain_name.cnf
openssl x509 -req -in $domain_name.csr -CA ca.cer -CAkey ca.key -set_serial $serial -extensions server -days 1024 -outform PEM -out $domain_name.cer -sha256 -extfile $domain_name.cnf
cat ca.cer >> $domain_name.cer
