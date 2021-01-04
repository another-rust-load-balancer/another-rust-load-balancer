#!/bin/sh
wrk2 -t 20 -c 1000 -d 60s -R 2000 --latency "http://127.0.0.1:8084" > benchmarks/whoami.txt
wrk2 -t 20 -c 1000 -d 60s -R 2000 --latency -H "Host: whoami.localhost" "http://127.0.0.1:8000" > benchmarks/whoami-traefik.txt
wrk2 -t 20 -c 1000 -d 60s -R 2000 --latency -H "Host: whoami.localhost" "http://127.0.0.1" > benchmarks/whoami-arlb.txt
