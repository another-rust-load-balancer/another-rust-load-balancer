#!/bin/bash

./generate-ca-certificate.sh
./generate-server-certificate.sh "https.localhost"
./generate-server-certificate.sh "www.arlb.de"
