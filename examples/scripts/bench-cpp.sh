#!/bin/sh

date
echo "nginx moderate"

cgexec -g cpuset:perfgauge --sticky \
  ./target/release/perf-gauge \
  --concurrency 10 \
  --rate 10000 --rate_step 1000 --rate_max 25000 \
  --max_iter 15 \
  --duration 60s \
  --name nginx-direct \
  --prometheus $PROMETHEUS_HOST:9091 \
  http http://localhost/10kb --conn_reuse --ignore_cert

sleep 4m

date
echo "haproxy moderate"

cgexec -g cpuset:perfgauge --sticky \
  ./target/release/perf-gauge \
  --concurrency 10 \
  --rate 10000 --rate_step 1000 --rate_max 25000 \
  --max_iter 15 \
  --duration 60s \
  --name nginx-direct \
  --prometheus $PROMETHEUS_HOST:9091 \
  http http://localhost:8999/10kb --conn_reuse

sleep 4m

date
echo "cpp moderate"

cgexec -g cpuset:perfgauge --sticky \
  ./target/release/perf-gauge \
  --concurrency 10 \
  --rate 10000 --rate_step 1000 --rate_max 25000 \
  --max_iter 15 \
  --duration 60s \
  --name nginx-direct \
  --prometheus $PROMETHEUS_HOST:9091 \
  http http://localhost:8081/10kb --conn_reuse --ignore_cert

sleep 4m

date
echo "rust moderate"
cgexec -g cpuset:perfgauge --sticky \
  ./target/release/perf-gauge \
  --concurrency 10 \
  --rate 10000 --rate_step 1000 --rate_max 25000 \
  --max_iter 15 \
  --duration 60s \
  --name nginx-direct \
  --prometheus $PROMETHEUS_HOST:9091 \
  http http://localhost:8080/10kb --conn_reuse --ignore_cert

sleep 4m

date
echo "golang moderate"
cgexec -g cpuset:perfgauge --sticky \
  ./target/release/perf-gauge \
  --concurrency 10 \
  --rate 10000 --rate_step 1000 --rate_max 25000 \
  --max_iter 15 \
  --duration 60s \
  --name nginx-direct \
  --prometheus $PROMETHEUS_HOST:9091 \
  http http://localhost:8111/10kb --conn_reuse --ignore_cert

sleep 4m

date
echo "nginx max"

cgexec -g cpuset:perfgauge --sticky \
  ./target/release/perf-gauge \
  --concurrency 100 \
  --max_iter 15 \
  --duration 60s \
  --name nginx-direct \
  --prometheus $PROMETHEUS_HOST:9091 \
  http http://localhost:80/10kb --conn_reuse --ignore_cert

sleep 4m

date
echo "haproxy max"

cgexec -g cpuset:perfgauge --sticky \
  ./target/release/perf-gauge \
  --concurrency 100 \
  --max_iter 15 \
  --duration 60s \
  --name nginx-direct \
  --prometheus $PROMETHEUS_HOST:9091 \
  http http://localhost:8999/10kb --conn_reuse --ignore_cert

sleep 4m

date
echo "cpp max"

cgexec -g cpuset:perfgauge --sticky \
  ./target/release/perf-gauge \
  --concurrency 100 \
  --max_iter 15 \
  --duration 60s \
  --name nginx-direct \
  --prometheus $PROMETHEUS_HOST:9091 \
  http http://localhost:8081/10kb --conn_reuse --ignore_cert

sleep 4m

date
echo "rust max"

cgexec -g cpuset:perfgauge --sticky \
  ./target/release/perf-gauge \
  --concurrency 100 \
  --max_iter 15 \
  --duration 60s \
  --name nginx-direct \
  --prometheus $PROMETHEUS_HOST:9091 \
  http http://localhost:8080/10kb --conn_reuse --ignore_cert

sleep 4m

date
echo "golang max"

cgexec -g cpuset:perfgauge --sticky \
  ./target/release/perf-gauge \
  --concurrency 100 \
  --max_iter 15 \
  --duration 60s \
  --name nginx-direct \
  --prometheus $PROMETHEUS_HOST:9091 \
  http http://localhost:8111/10kb --conn_reuse --ignore_cert

sleep 4m

date
echo "nginx no-keepalive"

cgexec -g cpuset:perfgauge --sticky \
  ./target/release/perf-gauge \
  --concurrency 10 \
  --rate 500 --rate_step 500 --rate_max 3500 \
  --max_iter 15 \
  --duration 60s \
  --name nginx-direct \
  --prometheus $PROMETHEUS_HOST:9091 \
  http http://localhost:80/10kb --ignore_cert

sleep 4m

date
echo "haproxy no-keepalive"

cgexec -g cpuset:perfgauge --sticky \
  ./target/release/perf-gauge \
  --concurrency 10 \
  --rate 500 --rate_step 500 --rate_max 3500 \
  --max_iter 15 \
  --duration 60s \
  --name nginx-direct \
  --prometheus $PROMETHEUS_HOST:9091 \
  http http://localhost:8999/10kb --ignore_cert

sleep 4m

date
echo "cpp no-keepalive"

cgexec -g cpuset:perfgauge --sticky \
  ./target/release/perf-gauge \
  --concurrency 10 \
  --rate 500 --rate_step 500 --rate_max 3500 \
  --max_iter 15 \
  --duration 60s \
  --name nginx-direct \
  --prometheus $PROMETHEUS_HOST:9091 \
  http http://localhost:8081/10kb --ignore_cert

sleep 4m

date
echo "rust no-keepalive"

cgexec -g cpuset:perfgauge --sticky \
  ./target/release/perf-gauge \
  --concurrency 10 \
  --rate 500 --rate_step 500 --rate_max 3500 \
  --max_iter 15 \
  --duration 60s \
  --name nginx-direct \
  --prometheus $PROMETHEUS_HOST:9091 \
  http http://localhost:8080/10kb --ignore_cert

sleep 4m

date
echo "golang no-keepalive"

cgexec -g cpuset:perfgauge --sticky \
  ./target/release/perf-gauge \
  --concurrency 10 \
  --rate 500 --rate_step 500 --rate_max 3500 \
  --max_iter 15 \
  --duration 60s \
  --name nginx-direct \
  --prometheus $PROMETHEUS_HOST:9091 \
  http http://localhost:8111/10kb --ignore_cert

sleep 4m
