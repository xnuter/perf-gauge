#!/bin/sh

sudo cgcreate -t $USER:$USER -a $USER:$USER -g cpuset:pyproxy2
echo 3 >/sys/fs/cgroup/cpuset/pyproxy2/cpuset.cpus
echo 0 >/sys/fs/cgroup/cpuset/pyproxy2/cpuset.mems

cgexec -g cpuset:pyproxy2 --sticky ~/.local/bin/pproxy -l tunnel://localhost:9001 -r tunnel://localhost:80
