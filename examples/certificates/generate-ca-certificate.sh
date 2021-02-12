#!/bin/bash

# Create our CA as described in https://www.makethenmakeinstall.com/2014/05/ssl-client-authentication-step-by-step/

openssl req -subj "/C=DE/ST=Deutschland/L=Muenchen/O=Another Rust Load Balancer/CN=Another Rust Load Balancer CA" -newkey rsa:4096 -keyform PEM -keyout ca.key -x509 -days 3650 -outform PEM -out ca.cer -nodes
