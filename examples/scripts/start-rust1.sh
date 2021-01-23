#!/bin/sh

cd ~/http-tunnel

sudo cgcreate -t $USER:$USER -a $USER:$USER  -g cpuset:rusttunnel
echo 2-3 > /sys/fs/cgroup/cpuset/rusttunnel/cpuset.cpus
echo 0 > /sys/fs/cgroup/cpuset/rusttunnel/cpuset.mems

cgexec -g cpuset:rusttunnel --sticky ./target/release/http-tunnel --bind 0.0.0.0:8080 tcp --destination localhost:80 

