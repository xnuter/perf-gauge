#!/bin/sh

date
echo "nginx no-keepalive"
cgexec -g cpuset:perfgauge --sticky \
        ./target/release/perf-gauge \
             --concurrency 10 \
             --rate 10 --rate_step 10 --rate_max 100 \
             --max_iter 25 \
             --duration 60s \
             --name nginx-direct \
             --prometheus $PROMETHEUS_HOST:9091 \
             http https://localhost/10kb --ignore_cert 

sleep 4m

date
echo "cpp no-keepalive"

cgexec -g cpuset:perfgauge --sticky \
        ./target/release/perf-gauge \
             --concurrency 10 \
             --rate 10 --rate_step 10 --rate_max 100 \
             --max_iter 25 \
             --duration 60s \
             --name nginx-direct \
             --prometheus $PROMETHEUS_HOST:9091 \
             http https://localhost/10kb --ignore_cert \
             --tunnel http://localhost:8081

sleep 4m

date
echo "rust no-keepalive"

cgexec -g cpuset:perfgauge --sticky \
        ./target/release/perf-gauge \
             --concurrency 10 \
             --rate 10 --rate_step 10 --rate_max 100 \
             --max_iter 25 \
             --duration 60s \
             --name nginx-direct \
             --prometheus $PROMETHEUS_HOST:9091 \
             http https://localhost/10kb --ignore_cert \
             --tunnel http://localhost:8080

sleep 4m
