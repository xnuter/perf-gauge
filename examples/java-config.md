### Running NetCrusher (Java)

Repository: https://github.com/NetCrusherOrg/netcrusher-java/

Empirically, I found out that you need to run two instances of NetCrusher to achieve higher density:

```bash
cd ~/netcrusher-core-0.10/bin

sudo cgcreate -t $USER:$USER -a $USER:$USER  -g cpuset:javanio1
echo 2-3 > /sys/fs/cgroup/cpuset/javanio1/cpuset.cpus
echo 0 > /sys/fs/cgroup/cpuset/javanio1/cpuset.mems

cgexec -g cpuset:javanio1 --sticky ./run-tcp-crusher.sh localhost:8000 localhost:80
```

and 
```bash
cd ~/netcrusher-core-0.10/bin

sudo cgcreate -t $USER:$USER -a $USER:$USER  -g cpuset:javanio2
echo 2-3 > /sys/fs/cgroup/cpuset/javanio2/cpuset.cpus
echo 0 > /sys/fs/cgroup/cpuset/javanio2/cpuset.mems

cgexec -g cpuset:javanio2 --sticky ./run-tcp-crusher.sh localhost:8001 localhost:80
```

### Starting up

I used `tmux` to be able to run and shutdown instances:

```bash
tmux new-session -d -s "java1" ./start-java1.sh # port 8000
tmux new-session -d -s "java2" ./start-java2.sh # port 8001
```

### Shutting down

```bash
tmux kill-session -t java1
tmux kill-session -t java2

# sometimes it still managed to survive, so to make sure it's killed
ps -ef | grep java | grep -v grep | awk {'print $2'} | xargs kill -9
```
