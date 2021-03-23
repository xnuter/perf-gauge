#!/bin/sh

sudo cgcreate -t $USER:$USER -a $USER:$USER -g cpuset:pyproxy1
echo 2 >/sys/fs/cgroup/cpuset/pyproxy1/cpuset.cpus
echo 0 >/sys/fs/cgroup/cpuset/pyproxy1/cpuset.mems

cgexec -g cpuset:pyproxy1 --sticky ~/.local/bin/pproxy -l tunnel://localhost:9000 -r tunnel://localhost:80
