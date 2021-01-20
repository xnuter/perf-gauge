### Setting up HAProxy

We need to specify TCP frontend and backend. It's important to turn off logging. Otherwise, it would flood the disk.
Also, it should only use cores #2 and #3: 

```
global
    # disable logging
    log /dev/log    local0 warning alert 
    log /dev/log    local1 warning alert
    chroot /var/lib/haproxy
    stats socket /run/haproxy/admin.sock mode 660 level admin expose-fd listeners
    stats timeout 30s
    user haproxy
    group haproxy
    # stick to cores 2 and 3
    nbproc 2
    cpu-map 1 2
    cpu-map 2 3
    daemon


frontend rserve_frontend
    bind *:8999
    mode tcp
    timeout client  1m
    default_backend rserve_backend

backend rserve_backend
    mode tcp
    option log-health-checks
    log global
    balance roundrobin
    timeout connect 10s
    timeout server 1m
    server rserve1 localhost:80
```

### Starting

```bash
sudo systemctl start haproxy
```

### Stopping
```bash
sudo systemctl stop haproxy
```