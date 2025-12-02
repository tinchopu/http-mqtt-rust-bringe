# Garage MQTT Bridge

A lightweight Rust-based HTTP-to-MQTT bridge service for controlling garage doors via iOS Shortcuts. This service runs in Kubernetes and securely forwards HTTP requests to an MQTT broker using client certificate authentication.

## Architecture

```
iOS Shortcut → HTTPS → Envoy (API Key) → Bridge Service → MQTT (TLS + Client Certs) → Garage Door
```

## Features

- 🦀 Written in Rust for performance and reliability
- 🔒 Secure MQTT connection with client certificate authentication
- 🔑 API key authentication via Envoy
- 🏥 Health check endpoint for Kubernetes probes
- 📦 Containerized and ready for Kubernetes deployment
- 🔄 Auto-reconnects to MQTT broker on connection loss

## Prerequisites

- Kubernetes cluster
- Docker (for building the image)
- MQTT broker accessible from the cluster
- Envoy for API key authentication

## Deployment Steps

### 1. Build the Docker Image

```bash
# Build the image
docker build -t garage-mqtt-bridge:latest .

# Tag for your registry (if using one)
docker tag garage-mqtt-bridge:latest your-registry/garage-mqtt-bridge:latest

# Push to your registry
docker push your-registry/garage-mqtt-bridge:latest
```

### 2. Prepare Certificates

Encode your certificates as base64:

```bash
# Generate base64 encoded certificates
cat ca.crt | base64 -w 0
cat client.crt | base64 -w 0
cat client.key | base64 -w 0
```

### 3. Configure Secrets

Edit `k8s/secrets.yaml` and add your base64-encoded certificates and API key:

```yaml
apiVersion: v1
kind: Secret
metadata:
  name: mqtt-certs
  namespace: default
type: Opaque
data:
  ca.crt: "<base64-encoded-ca-cert>"
  client.crt: "<base64-encoded-client-cert>"
  client.key: "<base64-encoded-client-key>"
---
apiVersion: v1
kind: Secret
metadata:
  name: garage-api-key
  namespace: default
type: Opaque
stringData:
  api-key: "your-secure-random-api-key-here"
```

Generate a secure API key:
```bash
openssl rand -hex 32
```

### 4. Deploy to Kubernetes

```bash
# Apply secrets
kubectl apply -f k8s/secrets.yaml

# Deploy the service
kubectl apply -f k8s/deployment.yaml
kubectl apply -f k8s/service.yaml

# Wait for the pod to be ready
kubectl wait --for=condition=ready pod -l app=garage-mqtt-bridge --timeout=60s
```

### 5. Deploy Envoy Gateway for API Key Authentication

Update the API key in `k8s/envoy-config.yaml` first (in the ConfigMap), then deploy:

```bash
kubectl apply -f k8s/envoy-config.yaml
```

This creates an Envoy gateway that:
- Validates the `x-api-key` header on all requests
- Returns 401 Unauthorized if the API key is missing or invalid
- Forwards valid requests to the garage-mqtt-bridge service
- Exposes a LoadBalancer service for external access

### 6. Test the Deployment

```bash
# First, test the bridge service directly (bypassing Envoy)
kubectl port-forward svc/garage-mqtt-bridge 8080:80
curl -X POST http://localhost:8080/garage
# Should return: {"status":"success","message":"Garage door triggered"}

# Test health endpoint
curl http://localhost:8080/health
# Should return: {"status":"healthy"}

# Now test through Envoy gateway with API key
kubectl port-forward svc/envoy-gateway 8081:80
curl -X POST http://localhost:8081/garage \
  -H "x-api-key: your-secure-api-key-here"
# Should return: {"status":"success","message":"Garage door triggered"}

# Test that invalid API key is rejected
curl -X POST http://localhost:8081/garage \
  -H "x-api-key: wrong-key"
# Should return: 401 Unauthorized

# Get the external endpoint for iOS
kubectl get svc envoy-gateway
```

### 7. Expose the Service

The Envoy gateway is deployed with a LoadBalancer service type. Get the external IP/hostname:

```bash
kubectl get svc envoy-gateway
```

The output will show something like:
```
NAME             TYPE           CLUSTER-IP      EXTERNAL-IP       PORT(S)
envoy-gateway    LoadBalancer   10.x.x.x        <pending/IP>      80:xxxxx/TCP
```

If you need to use a specific external IP or configure DNS:
- Wait for EXTERNAL-IP to be assigned
- Point your DNS record (e.g., `garage.your-domain.com`) to this IP
- Configure TLS termination at your load balancer or ingress if needed

Alternatively, if you prefer NodePort or ClusterIP, edit the service type in `k8s/envoy-config.yaml`.

## iOS Shortcuts Setup

### Create the Shortcut

1. Open **Shortcuts** app on iOS
2. Create a new shortcut
3. Add "Get Contents of URL" action
4. Configure:
   - **URL**: `https://your-cluster-domain/garage` (or your Envoy gateway endpoint)
   - **Method**: POST
   - **Headers**: Add `x-api-key` with value `your-secure-api-key-here`
5. (Optional) Add notification action to show success
6. Save with a name like "Öffne Garage" or "Open Garage"

### Example Shortcut Configuration

```
URL: https://garage.your-domain.com/garage
Method: POST
Headers:
  x-api-key: your-secure-api-key-here
```

### Add to Home Screen or Widget

You can add the shortcut to:
- Home screen as an icon
- Lock screen widget
- Siri voice command

## Configuration

### Environment Variables

The service supports the following environment variables (configured in `k8s/deployment.yaml`):

| Variable | Default | Description |
|----------|---------|-------------|
| `MQTT_HOST` | `mqtt.example.com` | MQTT broker hostname |
| `MQTT_PORT` | `8883` | MQTT broker port |
| `MQTT_TOPIC` | `garage/trigger` | MQTT topic to publish to |
| `MQTT_PAYLOAD` | `1` | Payload to send when triggered |
| `CA_CERT_PATH` | `/certs/ca.crt` | Path to CA certificate |
| `CLIENT_CERT_PATH` | `/certs/client.crt` | Path to client certificate |
| `CLIENT_KEY_PATH` | `/certs/client.key` | Path to client private key |
| `HTTP_PORT` | `8080` | HTTP server port |
| `RUST_LOG` | `info` | Log level (error, warn, info, debug, trace) |

### Update MQTT Configuration

If your MQTT broker details differ, update the environment variables in `k8s/deployment.yaml`:

```yaml
env:
- name: MQTT_HOST
  value: "your-mqtt-broker.com"
- name: MQTT_PORT
  value: "8883"
```

## Monitoring

### Check Logs

```bash
# View logs
kubectl logs -l app=garage-mqtt-bridge -f

# View Envoy logs (if using standalone)
kubectl logs -l app=envoy-gateway -f
```

### Health Check

The service exposes a `/health` endpoint:

```bash
curl http://your-service-url/health
```

### Kubernetes Probes

The deployment includes liveness and readiness probes that automatically check the `/health` endpoint.

## Troubleshooting

### Service won't start

1. Check certificate paths and validity:
   ```bash
   kubectl exec -it deployment/garage-mqtt-bridge -- ls -la /certs
   ```

2. Check logs for errors:
   ```bash
   kubectl logs -l app=garage-mqtt-bridge
   ```

### MQTT connection fails

1. Verify MQTT broker is reachable:
   ```bash
   kubectl exec -it deployment/garage-mqtt-bridge -- nc -zv your-mqtt-broker.com 8883
   ```

2. Check certificate validity:
   ```bash
   openssl x509 -in client.crt -text -noout
   ```

### API key authentication fails

1. Verify the API key matches in both the Secret and your iOS Shortcut
2. Check Envoy logs for authentication errors
3. Test without Envoy to isolate the issue:
   ```bash
   kubectl port-forward svc/garage-mqtt-bridge 8080:80
   curl -X POST http://localhost:8080/garage
   ```

### iOS Shortcut doesn't work

1. Test the endpoint from curl first
2. Check if the cluster endpoint is accessible from outside
3. Verify SSL/TLS certificates if using HTTPS
4. Check API key is correctly set in the header

## Security Considerations

- Always use HTTPS in production
- Generate strong random API keys (at least 32 bytes)
- Rotate API keys periodically
- Keep client certificates secure
- Use Kubernetes secrets for sensitive data
- Consider using cert-manager for certificate rotation
- Limit network policies to only necessary traffic

## Development

### Local Testing

Run locally without Kubernetes:

```bash
# Set environment variables
export MQTT_HOST=your-mqtt-broker.com
export MQTT_PORT=8883
export MQTT_TOPIC=garage/trigger
export MQTT_PAYLOAD=1
export CA_CERT_PATH=./ca.crt
export CLIENT_CERT_PATH=./client.crt
export CLIENT_KEY_PATH=./client.key
export RUST_LOG=debug

# Run the service
cargo run

# In another terminal, test it
curl -X POST http://localhost:8080/garage
```

### Build and Test Locally

```bash
# Build
cargo build --release

# Run tests (if any)
cargo test

# Run with debug logging
RUST_LOG=debug cargo run
```

## License

This project is provided as-is for personal use.
