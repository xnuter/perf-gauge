#!/bin/sh

cd ~/go/src/github.com/jpillora/go-tcp-proxy/cmd/tcp-proxy

sudo cgcreate -t $USER:$USER -a $USER:$USER -g cpuset:tcpproxy
echo 2-3 >/sys/fs/cgroup/cpuset/tcpproxy/cpuset.cpus
echo 0 >/sys/fs/cgroup/cpuset/tcpproxy/cpuset.mems

cgexec -g cpuset:tcpproxy --sticky ./tcp-proxy -l localhost:8111 -r localhost:80 >/dev/null
