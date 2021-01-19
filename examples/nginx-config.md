### Setting up Nginx

We'd like Nginx to exclusively utilize two cores:

```
worker_processes 2;
worker_cpu_affinity 00000001 00000010;
keepalive_requests 50;
```

Please note the `keepalive_requests` setting. We force reconnection every 50 requests, so `p99` would capture the connection establishment latency.
So each connection will serve `500kb` of data (`50 req * 10kb payload`).

### 10kb payload

We chose `10,000` bytes payload for a test request. It is human friendly (to make calculations of the throughput) and is a reasonable value for an average size of an RPC payload. 

```bash
cd /var/www/html
sudo openssl rand -out 10kb 10000
```
