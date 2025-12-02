# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Overview

This is a Rust-based HTTP-to-MQTT bridge service for controlling a garage door remotely via iOS Shortcuts. The service runs in Kubernetes and securely forwards HTTP POST requests to an MQTT broker using client certificate authentication.

## Architecture

```
iOS Shortcut → HTTPS → Envoy (API Key) → Bridge Service → MQTT (TLS + Client Certs) → Garage Door
```

### Components

- **Rust Service** (`src/main.rs`): HTTP server that receives POST requests and publishes to MQTT
  - Uses `actix-web` for HTTP server
  - Uses `rumqttc` for MQTT client with TLS support
  - Maintains persistent connection to MQTT broker with auto-reconnect

- **SSL/TLS Certificates**: Three certificate files for secure MQTT communication:
  - `ca.crt` - Certificate Authority certificate
  - `client.crt` - Client certificate for authentication
  - `client.key` - Private key for the client certificate (unencrypted)

- **Kubernetes Deployment**: Service runs as a pod in the cluster
  - Deployment: Runs the Rust service with certificates mounted as secrets
  - Service: ClusterIP service for internal cluster access
  - Envoy Gateway: Handles API key authentication and external access

## MQTT Configuration

The system connects to an MQTT broker with the following configuration (configure via environment variables):
- **Host**: Set via `MQTT_HOST` env var
- **Port**: Set via `MQTT_PORT` env var (typically 8883 for MQTT with TLS)
- **Topic**: Configured in the source code (default: configurable per deployment)
- **Message Payload**: "1" (triggers action)
- **Authentication**: Mutual TLS using client certificates

## Common Commands

### Development

```bash
# Build the Rust project
cargo build --release

# Run locally for testing
cargo run

# Run with debug logging
RUST_LOG=debug cargo run

# Run tests
cargo test
```

### Docker

```bash
# Build Docker image
docker build -t garage-mqtt-bridge:latest .

# Tag and push to registry
docker tag garage-mqtt-bridge:latest your-registry/garage-mqtt-bridge:latest
docker push your-registry/garage-mqtt-bridge:latest
```

### Kubernetes Deployment

```bash
# Encode certificates as base64 for secrets
cat ca.crt | base64 -w 0
cat client.crt | base64 -w 0
cat client.key | base64 -w 0

# Deploy to cluster
kubectl apply -f k8s/secrets.yaml
kubectl apply -f k8s/deployment.yaml
kubectl apply -f k8s/service.yaml
kubectl apply -f k8s/envoy-config.yaml

# Check status
kubectl get pods -l app=garage-mqtt-bridge
kubectl get pods -l app=envoy-gateway
kubectl logs -l app=garage-mqtt-bridge -f
kubectl logs -l app=envoy-gateway -f

# Get Envoy gateway external endpoint
kubectl get svc envoy-gateway

# Test the service (internal, bypassing Envoy)
kubectl port-forward svc/garage-mqtt-bridge 8080:80
curl -X POST http://localhost:8080/garage

# Test through Envoy
kubectl port-forward svc/envoy-gateway 8080:80
curl -X POST http://localhost:8080/garage -H "x-api-key: your-api-key"
```

### Testing MQTT Connection Manually

```bash
# Test MQTT connection with mosquitto_pub
mosquitto_pub --cafile ca.crt --cert client.crt --key client.key \
  -h your-mqtt-broker.com -p 8883 -t "your/topic" -m "1"
```

## Kubernetes Deployment Notes

- The service runs as a single replica (increase for HA if needed)
- Certificates are mounted from Kubernetes secrets
- Health checks ensure the service is ready before receiving traffic
- The service is exposed via ClusterIP and accessed through Envoy
- MQTT connection is maintained with auto-reconnect on failure
- Resource limits: 32Mi-128Mi memory, 50m-200m CPU

## iOS Shortcuts Integration

To trigger the garage door from iOS:

1. Create a new Shortcut in the iOS Shortcuts app
2. Add "Get Contents of URL" action with:
   - URL: `https://your-cluster-endpoint/garage`
   - Method: POST
   - Header: `x-api-key: your-secure-api-key`
3. The service responds with JSON: `{"status":"success","message":"Garage door triggered"}`

The main advantage over SSH is stable DNS-based access regardless of which Kubernetes node the pod runs on.

## Security Considerations

- Never commit certificates to git (see `.gitignore`)
- Use strong random API keys (32+ bytes)
- All MQTT communication is encrypted with TLS and mutual certificate authentication
- API key authentication is handled by Envoy at the edge
- Kubernetes secrets are used for sensitive data
- The client.key file should have restricted permissions (600) locally
