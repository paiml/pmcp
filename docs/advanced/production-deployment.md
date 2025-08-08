# Production Deployment Guide

## Overview

This guide covers best practices for deploying PMCP servers in production environments, including containerization, orchestration, monitoring, and scaling strategies. Following these guidelines ensures high availability, performance, and maintainability.

## Table of Contents

1. [Deployment Architecture](#deployment-architecture)
2. [Container Deployment](#container-deployment)
3. [Kubernetes Configuration](#kubernetes-configuration)
4. [Monitoring and Observability](#monitoring-and-observability)
5. [Scaling Strategies](#scaling-strategies)
6. [Security Hardening](#security-hardening)
7. [Performance Tuning](#performance-tuning)
8. [Disaster Recovery](#disaster-recovery)

## Deployment Architecture

### Single-Node Architecture

```
┌─────────────────┐
│   Load Balancer │
└────────┬────────┘
         │
┌────────▼────────┐
│   PMCP Server   │
│   (Container)   │
├─────────────────┤
│   Redis Cache   │
├─────────────────┤
│   PostgreSQL    │
└─────────────────┘
```

### Multi-Node Architecture

```
        ┌──────────────┐
        │Load Balancer │
        └──────┬───────┘
               │
    ┌──────────┼──────────┐
    │          │          │
┌───▼───┐ ┌───▼───┐ ┌───▼───┐
│ Node1 │ │ Node2 │ │ Node3 │
│ PMCP  │ │ PMCP  │ │ PMCP  │
└───┬───┘ └───┬───┘ └───┬───┘
    │          │          │
    └──────────┼──────────┘
               │
    ┌──────────┼──────────┐
    │          │          │
┌───▼───┐ ┌───▼───┐ ┌───▼───┐
│Redis  │ │Redis  │ │Redis  │
│Primary│ │Replica│ │Replica│
└───────┘ └───────┘ └───────┘
               │
        ┌──────▼──────┐
        │  PostgreSQL │
        │   Cluster   │
        └─────────────┘
```

## Container Deployment

### Docker Configuration

```dockerfile
# Multi-stage build for optimal size
FROM rust:1.75-slim as builder

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Create app directory
WORKDIR /usr/src/pmcp

# Copy manifests
COPY Cargo.toml Cargo.lock ./
COPY pmcp-macros/Cargo.toml ./pmcp-macros/

# Build dependencies (cached layer)
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release
RUN rm -rf src

# Copy source code
COPY . .

# Build application
RUN cargo build --release --bin pmcp-server

# Runtime stage
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN useradd -m -u 1001 -s /bin/bash pmcp

# Copy binary from builder
COPY --from=builder /usr/src/pmcp/target/release/pmcp-server /usr/local/bin/

# Copy configuration templates
COPY --from=builder /usr/src/pmcp/config /etc/pmcp/

# Set ownership
RUN chown -R pmcp:pmcp /etc/pmcp

# Switch to non-root user
USER pmcp

# Expose ports
EXPOSE 8080 9090

# Health check
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD ["/usr/local/bin/pmcp-server", "health"]

# Set entrypoint
ENTRYPOINT ["/usr/local/bin/pmcp-server"]
CMD ["serve", "--config", "/etc/pmcp/production.toml"]
```

### Docker Compose Example

```yaml
version: '3.8'

services:
  pmcp:
    image: pmcp:1.0.0
    container_name: pmcp-server
    restart: unless-stopped
    ports:
      - "8080:8080"  # HTTP API
      - "9090:9090"  # Metrics
    environment:
      - RUST_LOG=info,pmcp=debug
      - PMCP_ENV=production
      - DATABASE_URL=postgresql://pmcp:password@postgres:5432/pmcp
      - REDIS_URL=redis://redis:6379
    volumes:
      - ./config:/etc/pmcp:ro
      - pmcp-data:/var/lib/pmcp
    depends_on:
      postgres:
        condition: service_healthy
      redis:
        condition: service_healthy
    networks:
      - pmcp-network
    deploy:
      resources:
        limits:
          cpus: '2'
          memory: 2G
        reservations:
          cpus: '1'
          memory: 1G

  postgres:
    image: postgres:16-alpine
    container_name: pmcp-postgres
    restart: unless-stopped
    environment:
      - POSTGRES_USER=pmcp
      - POSTGRES_PASSWORD=password
      - POSTGRES_DB=pmcp
      - POSTGRES_INITDB_ARGS=--encoding=UTF8 --locale=C
    volumes:
      - postgres-data:/var/lib/postgresql/data
      - ./init.sql:/docker-entrypoint-initdb.d/init.sql:ro
    networks:
      - pmcp-network
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U pmcp"]
      interval: 10s
      timeout: 5s
      retries: 5

  redis:
    image: redis:7-alpine
    container_name: pmcp-redis
    restart: unless-stopped
    command: redis-server --appendonly yes --maxmemory 256mb --maxmemory-policy allkeys-lru
    volumes:
      - redis-data:/data
    networks:
      - pmcp-network
    healthcheck:
      test: ["CMD", "redis-cli", "ping"]
      interval: 10s
      timeout: 5s
      retries: 5

volumes:
  pmcp-data:
  postgres-data:
  redis-data:

networks:
  pmcp-network:
    driver: bridge
```

## Kubernetes Configuration

### Deployment Manifest

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: pmcp-server
  namespace: pmcp
  labels:
    app: pmcp
    component: server
spec:
  replicas: 3
  selector:
    matchLabels:
      app: pmcp
      component: server
  template:
    metadata:
      labels:
        app: pmcp
        component: server
      annotations:
        prometheus.io/scrape: "true"
        prometheus.io/port: "9090"
        prometheus.io/path: "/metrics"
    spec:
      serviceAccountName: pmcp-server
      securityContext:
        runAsNonRoot: true
        runAsUser: 1001
        fsGroup: 1001
      containers:
      - name: pmcp
        image: pmcp:1.0.0
        imagePullPolicy: IfNotPresent
        ports:
        - name: http
          containerPort: 8080
          protocol: TCP
        - name: metrics
          containerPort: 9090
          protocol: TCP
        env:
        - name: RUST_LOG
          value: "info,pmcp=debug"
        - name: POD_NAME
          valueFrom:
            fieldRef:
              fieldPath: metadata.name
        - name: POD_NAMESPACE
          valueFrom:
            fieldRef:
              fieldPath: metadata.namespace
        - name: DATABASE_URL
          valueFrom:
            secretKeyRef:
              name: pmcp-secrets
              key: database-url
        - name: REDIS_URL
          valueFrom:
            secretKeyRef:
              name: pmcp-secrets
              key: redis-url
        resources:
          requests:
            memory: "512Mi"
            cpu: "500m"
          limits:
            memory: "2Gi"
            cpu: "2000m"
        livenessProbe:
          httpGet:
            path: /health/live
            port: http
          initialDelaySeconds: 30
          periodSeconds: 10
          timeoutSeconds: 3
          failureThreshold: 3
        readinessProbe:
          httpGet:
            path: /health/ready
            port: http
          initialDelaySeconds: 5
          periodSeconds: 5
          timeoutSeconds: 3
          failureThreshold: 3
        volumeMounts:
        - name: config
          mountPath: /etc/pmcp
          readOnly: true
        - name: cache
          mountPath: /var/cache/pmcp
      volumes:
      - name: config
        configMap:
          name: pmcp-config
      - name: cache
        emptyDir:
          sizeLimit: 1Gi
      affinity:
        podAntiAffinity:
          preferredDuringSchedulingIgnoredDuringExecution:
          - weight: 100
            podAffinityTerm:
              labelSelector:
                matchExpressions:
                - key: app
                  operator: In
                  values:
                  - pmcp
              topologyKey: kubernetes.io/hostname
```

### Service Configuration

```yaml
apiVersion: v1
kind: Service
metadata:
  name: pmcp-server
  namespace: pmcp
  labels:
    app: pmcp
    component: server
spec:
  type: ClusterIP
  ports:
  - name: http
    port: 8080
    targetPort: http
    protocol: TCP
  - name: metrics
    port: 9090
    targetPort: metrics
    protocol: TCP
  selector:
    app: pmcp
    component: server
  sessionAffinity: ClientIP
  sessionAffinityConfig:
    clientIP:
      timeoutSeconds: 10800
```

### Horizontal Pod Autoscaler

```yaml
apiVersion: autoscaling/v2
kind: HorizontalPodAutoscaler
metadata:
  name: pmcp-server-hpa
  namespace: pmcp
spec:
  scaleTargetRef:
    apiVersion: apps/v1
    kind: Deployment
    name: pmcp-server
  minReplicas: 3
  maxReplicas: 10
  metrics:
  - type: Resource
    resource:
      name: cpu
      target:
        type: Utilization
        averageUtilization: 70
  - type: Resource
    resource:
      name: memory
      target:
        type: Utilization
        averageUtilization: 80
  - type: Pods
    pods:
      metric:
        name: pmcp_active_connections
      target:
        type: AverageValue
        averageValue: "100"
  behavior:
    scaleDown:
      stabilizationWindowSeconds: 300
      policies:
      - type: Percent
        value: 10
        periodSeconds: 60
      - type: Pods
        value: 1
        periodSeconds: 60
      selectPolicy: Min
    scaleUp:
      stabilizationWindowSeconds: 0
      policies:
      - type: Percent
        value: 100
        periodSeconds: 15
      - type: Pods
        value: 2
        periodSeconds: 15
      selectPolicy: Max
```

### Ingress Configuration

```yaml
apiVersion: networking.k8s.io/v1
kind: Ingress
metadata:
  name: pmcp-ingress
  namespace: pmcp
  annotations:
    kubernetes.io/ingress.class: nginx
    cert-manager.io/cluster-issuer: letsencrypt-prod
    nginx.ingress.kubernetes.io/rate-limit: "100"
    nginx.ingress.kubernetes.io/proxy-body-size: "10m"
    nginx.ingress.kubernetes.io/proxy-read-timeout: "3600"
    nginx.ingress.kubernetes.io/proxy-send-timeout: "3600"
spec:
  tls:
  - hosts:
    - api.pmcp.example.com
    secretName: pmcp-tls
  rules:
  - host: api.pmcp.example.com
    http:
      paths:
      - path: /
        pathType: Prefix
        backend:
          service:
            name: pmcp-server
            port:
              number: 8080
```

## Monitoring and Observability

### Prometheus Configuration

```yaml
# prometheus-config.yaml
global:
  scrape_interval: 15s
  evaluation_interval: 15s

scrape_configs:
  - job_name: 'pmcp'
    kubernetes_sd_configs:
    - role: pod
    relabel_configs:
    - source_labels: [__meta_kubernetes_pod_annotation_prometheus_io_scrape]
      action: keep
      regex: true
    - source_labels: [__meta_kubernetes_pod_annotation_prometheus_io_path]
      action: replace
      target_label: __metrics_path__
      regex: (.+)
    - source_labels: [__address__, __meta_kubernetes_pod_annotation_prometheus_io_port]
      action: replace
      regex: ([^:]+)(?::\d+)?;(\d+)
      replacement: $1:$2
      target_label: __address__
    - action: labelmap
      regex: __meta_kubernetes_pod_label_(.+)
    - source_labels: [__meta_kubernetes_namespace]
      action: replace
      target_label: kubernetes_namespace
    - source_labels: [__meta_kubernetes_pod_name]
      action: replace
      target_label: kubernetes_pod_name
```

### Grafana Dashboard

```json
{
  "dashboard": {
    "title": "PMCP Production Metrics",
    "panels": [
      {
        "title": "Request Rate",
        "targets": [
          {
            "expr": "rate(pmcp_requests_total[5m])"
          }
        ]
      },
      {
        "title": "Response Time",
        "targets": [
          {
            "expr": "histogram_quantile(0.95, rate(pmcp_request_duration_seconds_bucket[5m]))"
          }
        ]
      },
      {
        "title": "Active Connections",
        "targets": [
          {
            "expr": "pmcp_active_connections"
          }
        ]
      },
      {
        "title": "Error Rate",
        "targets": [
          {
            "expr": "rate(pmcp_errors_total[5m])"
          }
        ]
      }
    ]
  }
}
```

### Logging Configuration

```rust
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use opentelemetry::trace::TracerProvider;

pub fn init_observability() -> Result<()> {
    // OpenTelemetry tracer
    let tracer = opentelemetry_otlp::new_pipeline()
        .tracing()
        .with_exporter(
            opentelemetry_otlp::new_exporter()
                .tonic()
                .with_endpoint("http://otel-collector:4317")
        )
        .with_trace_config(
            opentelemetry::sdk::trace::config()
                .with_sampler(opentelemetry::sdk::trace::Sampler::AlwaysOn)
                .with_resource(opentelemetry::sdk::Resource::new(vec![
                    opentelemetry::KeyValue::new("service.name", "pmcp-server"),
                    opentelemetry::KeyValue::new("service.version", env!("CARGO_PKG_VERSION")),
                ]))
        )
        .install_batch(opentelemetry::runtime::Tokio)?;
    
    // Tracing subscriber
    let telemetry = tracing_opentelemetry::layer().with_tracer(tracer);
    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_target(false)
        .json();
    
    tracing_subscriber::registry()
        .with(telemetry)
        .with(fmt_layer)
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .init();
    
    Ok(())
}
```

## Scaling Strategies

### Horizontal Scaling

```rust
pub struct LoadBalancer {
    backends: Arc<RwLock<Vec<Backend>>>,
    strategy: BalancingStrategy,
}

impl LoadBalancer {
    pub async fn select_backend(&self) -> Result<Backend> {
        let backends = self.backends.read().await;
        
        match self.strategy {
            BalancingStrategy::RoundRobin => {
                // Round-robin selection
                static COUNTER: AtomicUsize = AtomicUsize::new(0);
                let idx = COUNTER.fetch_add(1, Ordering::Relaxed) % backends.len();
                Ok(backends[idx].clone())
            }
            BalancingStrategy::LeastConnections => {
                // Select backend with least connections
                backends
                    .iter()
                    .min_by_key(|b| b.active_connections())
                    .cloned()
                    .ok_or(Error::NoAvailableBackend)
            }
            BalancingStrategy::WeightedRandom => {
                // Weighted random selection
                use rand::distributions::WeightedIndex;
                use rand::prelude::*;
                
                let weights: Vec<_> = backends.iter().map(|b| b.weight()).collect();
                let dist = WeightedIndex::new(&weights)?;
                let mut rng = thread_rng();
                Ok(backends[dist.sample(&mut rng)].clone())
            }
        }
    }
}
```

### Vertical Scaling Configuration

```toml
# production.toml
[server]
worker_threads = 16
max_connections = 10000
connection_timeout = 30

[performance]
enable_simd = true
cache_size = "1GB"
buffer_size = "64MB"

[database]
pool_size = 50
connection_timeout = 10
idle_timeout = 300
max_lifetime = 3600

[redis]
pool_size = 20
connection_timeout = 5
```

## Security Hardening

### TLS Configuration

```rust
use rustls::{Certificate, PrivateKey, ServerConfig};
use rustls_pemfile::{certs, pkcs8_private_keys};

pub fn configure_tls() -> Result<ServerConfig> {
    // Load certificates
    let cert_file = std::fs::File::open("/etc/pmcp/tls/cert.pem")?;
    let key_file = std::fs::File::open("/etc/pmcp/tls/key.pem")?;
    
    let cert_chain = certs(&mut BufReader::new(cert_file))?
        .into_iter()
        .map(Certificate)
        .collect();
    
    let mut keys = pkcs8_private_keys(&mut BufReader::new(key_file))?;
    let key = PrivateKey(keys.remove(0));
    
    // Configure TLS
    let config = ServerConfig::builder()
        .with_safe_defaults()
        .with_no_client_auth()
        .with_single_cert(cert_chain, key)?;
    
    Ok(config)
}
```

### Security Headers

```rust
pub struct SecurityMiddleware;

#[async_trait]
impl Middleware for SecurityMiddleware {
    async fn process_request(
        &self,
        req: Request,
        next: Box<dyn Middleware>,
    ) -> Result<Response> {
        let mut response = next.process_request(req).await?;
        
        // Add security headers
        response.headers_mut().insert(
            "Strict-Transport-Security",
            "max-age=31536000; includeSubDomains".parse()?
        );
        response.headers_mut().insert(
            "X-Content-Type-Options",
            "nosniff".parse()?
        );
        response.headers_mut().insert(
            "X-Frame-Options",
            "DENY".parse()?
        );
        response.headers_mut().insert(
            "X-XSS-Protection",
            "1; mode=block".parse()?
        );
        response.headers_mut().insert(
            "Content-Security-Policy",
            "default-src 'self'".parse()?
        );
        
        Ok(response)
    }
}
```

## Performance Tuning

### System Configuration

```bash
#!/bin/bash
# system-tuning.sh

# Increase file descriptors
ulimit -n 65536

# TCP tuning
sysctl -w net.core.somaxconn=65536
sysctl -w net.ipv4.tcp_max_syn_backlog=65536
sysctl -w net.ipv4.ip_local_port_range="1024 65535"
sysctl -w net.ipv4.tcp_tw_reuse=1
sysctl -w net.ipv4.tcp_fin_timeout=30

# Memory tuning
sysctl -w vm.swappiness=10
sysctl -w vm.dirty_ratio=15
sysctl -w vm.dirty_background_ratio=5

# Network buffer tuning
sysctl -w net.core.rmem_max=134217728
sysctl -w net.core.wmem_max=134217728
sysctl -w net.ipv4.tcp_rmem="4096 87380 134217728"
sysctl -w net.ipv4.tcp_wmem="4096 65536 134217728"
```

### Application Profiling

```rust
use pprof::ProfilerGuard;

pub struct ProfilingMiddleware {
    profiler: Option<ProfilerGuard<'static>>,
}

impl ProfilingMiddleware {
    pub fn new(enable_profiling: bool) -> Self {
        let profiler = if enable_profiling {
            Some(pprof::ProfilerGuard::new(100).unwrap())
        } else {
            None
        };
        
        Self { profiler }
    }
    
    pub fn save_profile(&self) -> Result<()> {
        if let Some(profiler) = &self.profiler {
            let report = profiler.report().build()?;
            let file = std::fs::File::create("profile.pb")?;
            report.pprof()?;
        }
        Ok(())
    }
}
```

## Disaster Recovery

### Backup Strategy

```yaml
apiVersion: batch/v1
kind: CronJob
metadata:
  name: pmcp-backup
  namespace: pmcp
spec:
  schedule: "0 2 * * *"  # Daily at 2 AM
  jobTemplate:
    spec:
      template:
        spec:
          containers:
          - name: backup
            image: pmcp-backup:1.0.0
            command:
            - /bin/sh
            - -c
            - |
              # Backup database
              pg_dump $DATABASE_URL > /backup/pmcp-$(date +%Y%m%d).sql
              
              # Backup Redis
              redis-cli --rdb /backup/redis-$(date +%Y%m%d).rdb
              
              # Upload to S3
              aws s3 cp /backup/ s3://pmcp-backups/ --recursive
              
              # Clean old backups
              find /backup -mtime +30 -delete
            env:
            - name: DATABASE_URL
              valueFrom:
                secretKeyRef:
                  name: pmcp-secrets
                  key: database-url
            volumeMounts:
            - name: backup
              mountPath: /backup
          volumes:
          - name: backup
            persistentVolumeClaim:
              claimName: pmcp-backup-pvc
          restartPolicy: OnFailure
```

### Health Checks

```rust
pub async fn health_check_handler(
    State(app_state): State<AppState>,
) -> Result<impl IntoResponse> {
    let checks = vec![
        check_database(&app_state.db).await,
        check_redis(&app_state.redis).await,
        check_disk_space().await,
        check_memory_usage().await,
    ];
    
    let all_healthy = checks.iter().all(|c| c.is_ok());
    
    let status = if all_healthy {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    
    let body = json!({
        "status": if all_healthy { "healthy" } else { "unhealthy" },
        "checks": checks.into_iter().map(|c| {
            match c {
                Ok(name) => json!({ "name": name, "status": "ok" }),
                Err(e) => json!({ "name": e.component, "status": "error", "message": e.to_string() }),
            }
        }).collect::<Vec<_>>(),
        "timestamp": chrono::Utc::now(),
    });
    
    Ok((status, Json(body)))
}
```

## Production Checklist

### Pre-Deployment

- [ ] Security audit completed
- [ ] Load testing performed
- [ ] Backup strategy tested
- [ ] Monitoring dashboards configured
- [ ] Alerts configured
- [ ] Documentation updated
- [ ] Runbooks created

### Deployment

- [ ] Blue-green deployment setup
- [ ] Database migrations completed
- [ ] Feature flags configured
- [ ] Rollback plan documented
- [ ] Smoke tests prepared
- [ ] Communication plan ready

### Post-Deployment

- [ ] Monitor error rates
- [ ] Check performance metrics
- [ ] Verify backup jobs
- [ ] Review security logs
- [ ] Update status page
- [ ] Conduct retrospective

## Troubleshooting

### Common Issues

1. **High Memory Usage**
   - Check for memory leaks
   - Review cache configuration
   - Analyze heap dumps

2. **Connection Timeouts**
   - Verify network configuration
   - Check connection pool settings
   - Review firewall rules

3. **Performance Degradation**
   - Enable profiling
   - Check database query performance
   - Review middleware overhead

### Debug Commands

```bash
# Check pod logs
kubectl logs -n pmcp deployment/pmcp-server --tail=100

# Execute into pod
kubectl exec -it -n pmcp deployment/pmcp-server -- /bin/bash

# Check resource usage
kubectl top pods -n pmcp

# Describe pod events
kubectl describe pod -n pmcp -l app=pmcp

# Port forward for debugging
kubectl port-forward -n pmcp deployment/pmcp-server 8080:8080

# Check service endpoints
kubectl get endpoints -n pmcp
```

## Best Practices Summary

1. **Always use health checks** for load balancer integration
2. **Implement graceful shutdown** for zero-downtime deployments
3. **Use structured logging** for better observability
4. **Monitor key metrics** and set up alerts
5. **Regular security updates** and vulnerability scanning
6. **Test disaster recovery** procedures regularly
7. **Document everything** for operations team
8. **Use GitOps** for configuration management
9. **Implement rate limiting** to prevent abuse
10. **Regular performance testing** to catch regressions