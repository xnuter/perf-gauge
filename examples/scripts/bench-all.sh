#!/bin/sh

export PROMETHEUS_HOST=10.138.0.2

date
echo "nginx moderate"

cgexec -g cpuset:perfgauge --sticky \
  ./target/release/perf-gauge \
  --concurrency 10 \
  --rate 10000 --rate_step 1000 --rate_max 25000 \
  --max_iter 10 \
  --duration 60s \
  --name nginx-direct \
  --prometheus $PROMETHEUS_HOST:9091 \
  http http://localhost/10kb --conn_reuse

sleep 2m

date
echo "haproxy moderate"

./start-c.sh

cgexec -g cpuset:perfgauge --sticky \
  ./target/release/perf-gauge \
  --concurrency 10 \
  --rate 10000 --rate_step 1000 --rate_max 25000 \
  --max_iter 10 \
  --duration 60s \
  --name nginx-direct \
  --prometheus $PROMETHEUS_HOST:9091 \
  http http://localhost:8999/10kb --conn_reuse

sleep 1m
./stop-c.sh
sleep 1m

./start-cpp.sh

date
echo "cpp moderate"

cgexec -g cpuset:perfgauge --sticky \
  ./target/release/perf-gauge \
  --concurrency 10 \
  --rate 10000 --rate_step 1000 --rate_max 25000 \
  --max_iter 10 \
  --duration 60s \
  --name nginx-direct \
  --prometheus $PROMETHEUS_HOST:9091 \
  http http://localhost:8081/10kb --conn_reuse

sleep 1m
./stop-cpp.sh
sleep 1m

date
echo "rust moderate"
./start-rust.sh

cgexec -g cpuset:perfgauge --sticky \
  ./target/release/perf-gauge \
  --concurrency 10 \
  --rate 10000 --rate_step 1000 --rate_max 25000 \
  --max_iter 10 \
  --duration 60s \
  --name nginx-direct \
  --prometheus $PROMETHEUS_HOST:9091 \
  http http://localhost:8080/10kb --conn_reuse

sleep 1m
./stop-rust.sh
sleep 1m

./start-go.sh
date
echo "golang moderate"
cgexec -g cpuset:perfgauge --sticky \
  ./target/release/perf-gauge \
  --concurrency 10 \
  --rate 10000 --rate_step 1000 --rate_max 25000 \
  --max_iter 10 \
  --duration 60s \
  --name nginx-direct \
  --prometheus $PROMETHEUS_HOST:9091 \
  http http://localhost:8111/10kb --conn_reuse

sleep 1m
./stop-go.sh
sleep 1m

./start-java.sh

date
echo "java moderate"
cgexec -g cpuset:perfgauge --sticky \
  ./target/release/perf-gauge \
  --concurrency 10 \
  --rate 10000 --rate_step 1000 --rate_max 20000 \
  --max_iter 10 \
  --duration 60s \
  --name nginx-direct \
  --prometheus $PROMETHEUS_HOST:9091 \
  http http://localhost:8000/10kb http://localhost:8001/10kb --conn_reuse

sleep 1m
./stop-java.sh
sleep 1m

./start-py.sh
date
echo "python moderate"
cgexec -g cpuset:perfgauge --sticky \
  ./target/release/perf-gauge \
  --concurrency 10 \
  --rate 5000 --rate_step 1000 --rate_max 15000 \
  --max_iter 10 \
  --duration 60s \
  --name nginx-direct \
  --prometheus $PROMETHEUS_HOST:9091 \
  http http://localhost:9000/10kb http://localhost:9001/10kb --conn_reuse

sleep 1m
./stop-py.sh
sleep 1m

date
echo "nginx max"

cgexec -g cpuset:perfgauge --sticky \
  ./target/release/perf-gauge \
  --concurrency 100 \
  --max_iter 10 \
  --duration 60s \
  --name nginx-direct \
  --prometheus $PROMETHEUS_HOST:9091 \
  http http://localhost:80/10kb --conn_reuse

sleep 2m

date
echo "haproxy max"
./start-c.sh

cgexec -g cpuset:perfgauge --sticky \
  ./target/release/perf-gauge \
  --concurrency 100 \
  --max_iter 10 \
  --duration 60s \
  --name nginx-direct \
  --prometheus $PROMETHEUS_HOST:9091 \
  http http://localhost:8999/10kb --conn_reuse

sleep 1m
./stop-c.sh
sleep 1m

./start-cpp.sh

date
echo "cpp max"

cgexec -g cpuset:perfgauge --sticky \
  ./target/release/perf-gauge \
  --concurrency 100 \
  --max_iter 10 \
  --duration 60s \
  --name nginx-direct \
  --prometheus $PROMETHEUS_HOST:9091 \
  http http://localhost:8081/10kb --conn_reuse

sleep 1m
./stop-cpp.sh
sleep 1m

./start-rust.sh
date
echo "rust max"

cgexec -g cpuset:perfgauge --sticky \
  ./target/release/perf-gauge \
  --concurrency 100 \
  --max_iter 10 \
  --duration 60s \
  --name nginx-direct \
  --prometheus $PROMETHEUS_HOST:9091 \
  http http://localhost:8080/10kb --conn_reuse

sleep 1m
./stop-rust.sh
sleep 1m

./start-go.sh
date
echo "golang max"

cgexec -g cpuset:perfgauge --sticky \
  ./target/release/perf-gauge \
  --concurrency 100 \
  --max_iter 10 \
  --duration 60s \
  --name nginx-direct \
  --prometheus $PROMETHEUS_HOST:9091 \
  http http://localhost:8111/10kb --conn_reuse

sleep 1m
./stop-go.sh
sleep 1m

./start-java.sh
date
echo "java max"

cgexec -g cpuset:perfgauge --sticky \
  ./target/release/perf-gauge \
  --concurrency 100 \
  --max_iter 10 \
  --duration 60s \
  --name nginx-direct \
  --prometheus $PROMETHEUS_HOST:9091 \
  http http://localhost:8000/10kb http://localhost:8001/10kb --conn_reuse

sleep 1m
./stop-java.sh
sleep 1m

./start-py.sh
date
echo "python max"

cgexec -g cpuset:perfgauge --sticky \
  ./target/release/perf-gauge \
  --concurrency 100 \
  --max_iter 10 \
  --duration 60s \
  --name nginx-direct \
  --prometheus $PROMETHEUS_HOST:9091 \
  http http://localhost:9000/10kb http://localhost:9001/10kb --conn_reuse

sleep 1m
./stop-py.sh
sleep 1m

date
echo "nginx no-keepalive"

cgexec -g cpuset:perfgauge --sticky \
  ./target/release/perf-gauge \
  --concurrency 10 \
  --rate 500 --rate_step 500 --rate_max 3500 \
  --max_iter 10 \
  --duration 60s \
  --name nginx-direct \
  --prometheus $PROMETHEUS_HOST:9091 \
  http http://localhost:80/10kb

sleep 2m

./start-c.sh

date
echo "haproxy no-keepalive"

cgexec -g cpuset:perfgauge --sticky \
  ./target/release/perf-gauge \
  --concurrency 10 \
  --rate 500 --rate_step 500 --rate_max 3500 \
  --max_iter 10 \
  --duration 60s \
  --name nginx-direct \
  --prometheus $PROMETHEUS_HOST:9091 \
  http http://localhost:8999/10kb

sleep 1m
./stop-c.sh
sleep 1m

./start-cpp.sh

date
echo "cpp no-keepalive"

cgexec -g cpuset:perfgauge --sticky \
  ./target/release/perf-gauge \
  --concurrency 10 \
  --rate 500 --rate_step 500 --rate_max 3500 \
  --max_iter 10 \
  --duration 60s \
  --name nginx-direct \
  --prometheus $PROMETHEUS_HOST:9091 \
  http http://localhost:8081/10kb

sleep 1m
./stop-cpp.sh
sleep 1m

./start-rust.sh
date
echo "rust no-keepalive"

cgexec -g cpuset:perfgauge --sticky \
  ./target/release/perf-gauge \
  --concurrency 10 \
  --rate 500 --rate_step 500 --rate_max 3500 \
  --max_iter 10 \
  --duration 60s \
  --name nginx-direct \
  --prometheus $PROMETHEUS_HOST:9091 \
  http http://localhost:8080/10kb

sleep 1m
./stop-rust.sh
sleep 1m

./start-go.sh
date
echo "golang no-keepalive"

cgexec -g cpuset:perfgauge --sticky \
  ./target/release/perf-gauge \
  --concurrency 10 \
  --rate 500 --rate_step 500 --rate_max 3500 \
  --max_iter 10 \
  --duration 60s \
  --name nginx-direct \
  --prometheus $PROMETHEUS_HOST:9091 \
  http http://localhost:8111/10kb

sleep 1m
./stop-go.sh
sleep 1m

./start-java.sh

date
echo "java no-keepalive"

cgexec -g cpuset:perfgauge --sticky \
  ./target/release/perf-gauge \
  --concurrency 10 \
  --rate 500 --rate_step 250 --rate_max 1500 \
  --max_iter 10 \
  --duration 60s \
  --name nginx-direct \
  --prometheus $PROMETHEUS_HOST:9091 \
  http http://localhost:8000/10kb http://localhost:8001/10kb

sleep 1m
./stop-java.sh
sleep 1m

./start-py.sh

echo "python no-keepalive"

cgexec -g cpuset:perfgauge --sticky \
  ./target/release/perf-gauge \
  --concurrency 10 \
  --rate 500 --rate_step 250 --rate_max 1500 \
  --max_iter 10 \
  --duration 60s \
  --name nginx-direct \
  --prometheus $PROMETHEUS_HOST:9091 \
  http http://localhost:9000/10kb http://localhost:9001/10kb

sleep 1m

./stop-py.sh

sleep 1m
