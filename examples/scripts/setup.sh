#!/bin/sh

sudo cgcreate -t $USER:$USER -a $USER:$USER  -g cpuset:perfgauge
echo 4-7 > /sys/fs/cgroup/cpuset/perfgauge/cpuset.cpus
echo 0 > /sys/fs/cgroup/cpuset/perfgauge/cpuset.mems


