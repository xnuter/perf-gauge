#!/bin/sh

cd ~/netcrusher-core-0.10/bin

sudo cgcreate -t $USER:$USER -a $USER:$USER -g cpuset:javanio1
echo 2-3 >/sys/fs/cgroup/cpuset/javanio1/cpuset.cpus
echo 0 >/sys/fs/cgroup/cpuset/javanio1/cpuset.mems

cgexec -g cpuset:javanio1 --sticky ~/netcrusher-core-0.10/bin/run-tcp-crusher.sh localhost:8000 localhost:80
