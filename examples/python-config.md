### Running pproxy (Python)

Repository: https://pypi.org/project/pproxy/

`pproxy` is a single-threaded `asyncio` app, so to utilize two cores, we need to run two instances.

```bash
sudo cgcreate -t $USER:$USER -a $USER:$USER  -g cpuset:pyproxy1
echo 2 > /sys/fs/cgroup/cpuset/pyproxy1/cpuset.cpus
echo 0 > /sys/fs/cgroup/cpuset/pyproxy1/cpuset.mems

cgexec -g cpuset:pyproxy1 --sticky   ~/.local/bin/pproxy -l tunnel://localhost:9000 -r tunnel://localhost:80
```

and

```bash
sudo cgcreate -t $USER:$USER -a $USER:$USER  -g cpuset:pyproxy2
echo 2 > /sys/fs/cgroup/cpuset/pyproxy2/cpuset.cpus
echo 0 > /sys/fs/cgroup/cpuset/pyproxy2/cpuset.mems

cgexec -g cpuset:pyproxy2 --sticky   ~/.local/bin/pproxy -l tunnel://localhost:9001 -r tunnel://localhost:80
```

### Starting up

I used `tmux` to be able to run and shutdown instances:

```bash
tmux new-session -d -s "python1" ./start-python1.sh # port 9000
tmux new-session -d -s "python2" ./start-python2.sh # port 9001
```

### Shutting down

```bash
tmux kill-session -t python1
tmux kill-session -t python2
```
