#!/bin/bash

# Create our CA and sign a server certificate as described in https://www.makethenmakeinstall.com/2014/05/ssl-client-authentication-step-by-step/ and https://security.stackexchange.com/a/176084.

mkdir x509
cd x509
openssl req -subj "/C=DE/ST=Deutschland/L=Muenchen/O=Another Rust Load Balancer/CN=Another Rust Load Balancer CA" -newkey rsa:4096 -keyform PEM -keyout ca.key -x509 -days 3650 -outform PEM -out ca.cer -nodes

openssl genrsa -out localhost.key 4096
openssl req -new -subj "/C=DE/ST=Deutschland/L=Muenchen/O=Another Rust Load Balancer/CN=localhost" -addext "subjectAltName=DNS:localhost" -key localhost.key -out localhost.csr -sha256
echo "[server]
subjectAltName=DNS:localhost
" > localhost.cnf
openssl x509 -req -in localhost.csr -CA ca.cer -CAkey ca.key -set_serial 1 -extensions server -days 1024 -outform PEM -out localhost.cer -sha256 -extfile localhost.cnf
cat ca.cer >> localhost.cer

openssl genrsa -out www.arlb.de.key 4096
openssl req -new -subj "/C=DE/ST=Deutschland/L=Muenchen/O=Another Rust Load Balancer/CN=www.arlb.de" -addext "subjectAltName=DNS:www.arlb.de" -key www.arlb.de.key -out www.arlb.de.csr -sha256
echo "[server]
subjectAltName=DNS:www.arlb.de
" > www.arlb.de.cnf
openssl x509 -req -in www.arlb.de.csr -CA ca.cer -CAkey ca.key -set_serial 2 -extensions server -days 1024 -outform PEM -out www.arlb.de.cer -sha256 -extfile www.arlb.de.cnf
cat ca.cer >> www.arlb.de.cer

openssl genrsa -out https.localhost.key 4096
openssl req -new -subj "/C=DE/ST=Deutschland/L=Muenchen/O=Another Rust Load Balancer/CN=https.localhost" -addext "subjectAltName=DNS:https.localhost" -key https.localhost.key -out https.localhost.csr -sha256
echo "[server]
subjectAltName=DNS:https.localhost
" > https.localhost.cnf
openssl x509 -req -in https.localhost.csr -CA ca.cer -CAkey ca.key -set_serial 2 -extensions server -days 1024 -outform PEM -out https.localhost.cer -sha256 -extfile https.localhost.cnf
cat ca.cer >> https.localhost.cer
