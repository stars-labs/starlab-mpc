# Deployment Guide

## Overview

Complete guide for deploying MPC Wallet components to production
environments. This single-page guide covers:

- Quick-start deployment recipes (Docker, Cloudflare Worker,
  browser extension)
- Production architecture and infrastructure sizing
- Kubernetes and Docker Compose manifests
- Monitoring, security hardening, backup & recovery
- Linux kernel tuning + signal-server tuning parameters
- Pre/during/post-deployment checklist
- Troubleshooting recipes

For the Cloudflare Worker deployment specifically, see the
dedicated guide: [CLOUDFLARE_DEPLOYMENT.md](CLOUDFLARE_DEPLOYMENT.md).

## Quick Deployment

### Signal Server (Docker)

```bash
# Build Docker image
docker build -t mpc-signal-server apps/signal-server/server

# Run with Docker Compose
docker-compose up -d

# Or run directly
docker run -d \
  -p 8080:8080 \
  -e RUST_LOG=info \
  --name signal-server \
  mpc-signal-server
```

### Cloudflare Worker

```bash
cd apps/signal-server/cloudflare-worker

# Configure wrangler
wrangler login

# Deploy to Cloudflare
wrangler publish

# View logs
wrangler tail
```

### Browser Extension

```bash
# Build for production
cd apps/browser-extension
bun run build:chrome

# Create distribution package
cd .output/chrome-mv3
zip -r ../../mpc-wallet-chrome.zip .

# Upload to Chrome Web Store Developer Dashboard
```

## Production Architecture

### Recommended Setup

```
                    ┌─────────────────┐
                    │   Load Balancer │
                    │   (AWS ALB/NLB)  │
                    └────────┬────────┘
                             │
                ┌────────────┼────────────┐
                │            │            │
         ┌──────▼────┐ ┌────▼──────┐ ┌──▼────────┐
         │  Signal   │ │  Signal   │ │  Signal   │
         │  Server 1 │ │  Server 2 │ │  Server 3 │
         └───────────┘ └───────────┘ └───────────┘
                │            │            │
                └────────────┼────────────┘
                             │
                    ┌────────▼────────┐
                    │   Redis Cache   │
                    │  (Session State) │
                    └─────────────────┘
```

### Infrastructure Requirements

#### Signal Server
- **CPU**: 4 vCPUs (minimum)
- **Memory**: 8GB RAM
- **Storage**: 50GB SSD
- **Network**: 1Gbps
- **Instances**: 3+ for HA

#### STUN/TURN Servers
- **Bandwidth**: 10Gbps
- **Locations**: Multiple regions
- **Provider**: Coturn or commercial

#### Database (Optional)
- **Type**: PostgreSQL 14+
- **Storage**: 100GB+
- **Backup**: Daily snapshots

## Deployment Configurations

### Environment Variables

```bash
# Signal Server
RUST_LOG=info
PORT=8080
REDIS_URL=redis://localhost:6379
MAX_CONNECTIONS=10000
SESSION_TIMEOUT=3600

# STUN/TURN
STUN_SERVER=stun:stun.example.com:3478
TURN_SERVER=turn:turn.example.com:3478
TURN_USERNAME=username
TURN_PASSWORD=password
```

### Kubernetes Deployment

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: signal-server
spec:
  replicas: 3
  selector:
    matchLabels:
      app: signal-server
  template:
    metadata:
      labels:
        app: signal-server
    spec:
      containers:
      - name: signal-server
        image: mpc-wallet/signal-server:latest
        ports:
        - containerPort: 8080
        env:
        - name: RUST_LOG
          value: "info"
        resources:
          requests:
            memory: "4Gi"
            cpu: "2"
          limits:
            memory: "8Gi"
            cpu: "4"
```

### Docker Compose

```yaml
version: '3.8'

services:
  signal-server:
    image: mpc-wallet/signal-server:latest
    ports:
      - "8080:8080"
    environment:
      - RUST_LOG=info
      - REDIS_URL=redis://redis:6379
    depends_on:
      - redis
    restart: unless-stopped

  redis:
    image: redis:7-alpine
    ports:
      - "6379:6379"
    volumes:
      - redis-data:/data
    restart: unless-stopped

  nginx:
    image: nginx:alpine
    ports:
      - "443:443"
      - "80:80"
    volumes:
      - ./nginx.conf:/etc/nginx/nginx.conf
      - ./certs:/etc/nginx/certs
    depends_on:
      - signal-server
    restart: unless-stopped

volumes:
  redis-data:
```

## Monitoring & Observability

### Prometheus Metrics

```yaml
# prometheus.yml
global:
  scrape_interval: 15s

scrape_configs:
  - job_name: 'signal-server'
    static_configs:
      - targets: ['localhost:9090']
```

### Grafana Dashboard

Key metrics to monitor:
- Active connections
- Message throughput
- Session creation rate
- Error rate
- P95 latency

### Logging

```bash
# Structured logging with Vector
[sources.signal_server]
type = "docker_logs"
include_containers = ["signal-server"]

[transforms.parse]
type = "remap"
inputs = ["signal_server"]
source = '''
. = parse_json!(.message)
'''

[sinks.elasticsearch]
type = "elasticsearch"
inputs = ["parse"]
endpoint = "https://elasticsearch:9200"
```

## Security Hardening

### TLS Configuration

```nginx
server {
    listen 443 ssl http2;
    ssl_certificate /etc/nginx/certs/cert.pem;
    ssl_certificate_key /etc/nginx/certs/key.pem;
    
    ssl_protocols TLSv1.2 TLSv1.3;
    ssl_ciphers HIGH:!aNULL:!MD5;
    ssl_prefer_server_ciphers on;
    
    location /ws {
        proxy_pass http://signal-server:8080;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
    }
}
```

### Firewall Rules

```bash
# Allow only necessary ports
ufw allow 22/tcp   # SSH
ufw allow 443/tcp  # HTTPS
ufw allow 8080/tcp # WebSocket
ufw enable
```

## Backup & Recovery

### Backup Strategy

```bash
#!/bin/bash
# backup.sh

# Backup Redis data
redis-cli --rdb /backup/redis-$(date +%Y%m%d).rdb

# Backup configuration
tar -czf /backup/config-$(date +%Y%m%d).tar.gz /etc/mpc-wallet

# Upload to S3
aws s3 cp /backup/ s3://mpc-wallet-backups/ --recursive
```

### Recovery Procedure

1. Restore Redis data
2. Restore configuration files
3. Start services in order
4. Verify connectivity
5. Run health checks

## Performance Tuning

### Linux Kernel Parameters

```bash
# /etc/sysctl.conf
net.core.somaxconn = 65535
net.ipv4.tcp_max_syn_backlog = 65535
net.core.netdev_max_backlog = 65535
fs.file-max = 2097152
```

### Signal Server Tuning

```rust
// Increase connection pool
const MAX_CONNECTIONS: usize = 10000;
const CONNECTION_TIMEOUT: Duration = Duration::from_secs(60);
```

## Deployment Checklist

### Pre-Deployment
- [ ] Security audit completed
- [ ] Load testing performed
- [ ] Backup strategy tested
- [ ] Monitoring configured
- [ ] Documentation updated

### Deployment
- [ ] Deploy to staging environment
- [ ] Run integration tests
- [ ] Perform smoke tests
- [ ] Deploy to production
- [ ] Monitor metrics

### Post-Deployment
- [ ] Verify all services healthy
- [ ] Check error rates
- [ ] Review performance metrics
- [ ] Update status page
- [ ] Notify stakeholders

## Troubleshooting

### Common Issues

#### High Memory Usage
```bash
# Check memory usage
docker stats signal-server

# Increase memory limit
docker update --memory 16g signal-server
```

#### Connection Failures
```bash
# Check firewall rules
iptables -L -n

# Test WebSocket connection
wscat -c ws://localhost:8080
```

#### Performance Issues
```bash
# Profile CPU usage
perf top -p $(pgrep signal-server)

# Check network latency
mtr xiongchenyu.dpdns.org
```

## Navigation

- [← Back to Main Documentation](../README.md)
- [Testing Guide →](../testing/README.md)