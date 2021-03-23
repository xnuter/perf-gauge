#!/bin/sh

cd ~/cpptunnel/draft-http-tunnel

sudo cgcreate -t $USER:$USER -a $USER:$USER -g cpuset:cpptunnel
echo 2-3 >/sys/fs/cgroup/cpuset/cpptunnel/cpuset.cpus
echo 0 >/sys/fs/cgroup/cpuset/cpptunnel/cpuset.mems

cgexec -g cpuset:cpptunnel --sticky ./draft_http_tunnel --bind 0.0.0.0:8081 tcp --destination localhost:80 >/dev/null
