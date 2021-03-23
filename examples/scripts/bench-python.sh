#!/bin/sh

date
echo "java moderate"
cgexec -g cpuset:perfgauge --sticky \
  ./target/release/perf-gauge \
  --concurrency 10 \
  --rate 10000 --rate_step 1000 --rate_max 25000 \
  --max_iter 15 \
  --duration 60s \
  --name nginx-direct \
  --prometheus $PROMETHEUS_HOST:9091 \
  http http://localhost:9000/10kb http://localhost:9001/10kb --conn_reuse --ignore_cert

sleep 4m

date
echo "java max"

cgexec -g cpuset:perfgauge --sticky \
  ./target/release/perf-gauge \
  --concurrency 100 \
  --max_iter 15 \
  --duration 60s \
  --name nginx-direct \
  --prometheus $PROMETHEUS_HOST:9091 \
  http http://localhost:9000/10kb http://localhost:9001/10kb --conn_reuse --ignore_cert

sleep 4m

date
echo "java no-keepalive"

cgexec -g cpuset:perfgauge --sticky \
  ./target/release/perf-gauge \
  --concurrency 10 \
  --rate 500 --rate_step 500 --rate_max 3500 \
  --max_iter 15 \
  --duration 60s \
  --name nginx-direct \
  --prometheus $PROMETHEUS_HOST:9091 \
  http http://localhost:9000/10kb --ignore_cert

sleep 4m
