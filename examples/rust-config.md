### Running http-tunnel (Rust)

Repository: https://github.com/xnuter/http-tunnel/

```bash
sudo cgcreate -t $USER:$USER -a $USER:$USER  -g cpuset:rusttunnel
echo 2-3 > /sys/fs/cgroup/cpuset/rusttunnel/cpuset.cpus
echo 0 > /sys/fs/cgroup/cpuset/rusttunnel/cpuset.mems

cgexec -g cpuset:rusttunnel --sticky http-tunnel --bind 0.0.0.0:8080 tcp --destination localhost:80 
```

To disable logging in the `./config/log4rs.yaml` change the `level` from `info` to `error`:

```yaml
root:
  level: error
  appenders:
    - application

loggers:
  metrics:
    level: error 
    appenders:
      - metrics
    additive: false
```