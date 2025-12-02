# Quick Start Guide

Get your garage door HTTP-to-MQTT bridge running in 5 minutes!

## Prerequisites Checklist

- [ ] Kubernetes cluster access
- [ ] `kubectl` configured
- [ ] Docker installed (for building)
- [ ] Your MQTT certificates: `ca.crt`, `client.crt`, `client.key`

## Step 1: Generate API Key

```bash
openssl rand -hex 32
```

Save this API key - you'll need it for both Kubernetes and iOS!

## Step 2: Prepare Certificates

```bash
# Base64 encode your certificates
echo "ca.crt:"; cat ca.crt | base64 -w 0; echo
echo "client.crt:"; cat client.crt | base64 -w 0; echo
echo "client.key:"; cat client.key | base64 -w 0; echo
```

## Step 3: Update Secrets

Edit `k8s/secrets.yaml` and paste:
- Your base64-encoded certificates
- Your generated API key

```yaml
data:
  ca.crt: "<paste here>"
  client.crt: "<paste here>"
  client.key: "<paste here>"
```

```yaml
stringData:
  api-key: "<paste your API key here>"
```

## Step 4: Build and Push Image

```bash
# Build
docker build -t garage-mqtt-bridge:latest .

# If using a registry, tag and push
docker tag garage-mqtt-bridge:latest your-registry/garage-mqtt-bridge:latest
docker push your-registry/garage-mqtt-bridge:latest

# Update image in k8s/deployment.yaml if needed
```

## Step 5: Deploy to Kubernetes

```bash
kubectl apply -f k8s/secrets.yaml
kubectl apply -f k8s/deployment.yaml
kubectl apply -f k8s/service.yaml
```

Wait for the pod to be ready:
```bash
kubectl wait --for=condition=ready pod -l app=garage-mqtt-bridge --timeout=60s
```

## Step 6: Deploy Envoy Gateway

1. Edit `k8s/envoy-config.yaml` - update the API key in the ConfigMap (search for `your-secure-api-key-here`)
2. Apply: `kubectl apply -f k8s/envoy-config.yaml`
3. Wait for Envoy to be ready: `kubectl wait --for=condition=ready pod -l app=envoy-gateway --timeout=60s`
4. Get the LoadBalancer IP: `kubectl get svc envoy-gateway`

## Step 7: Test It!

```bash
# Test through Envoy with API key
kubectl port-forward svc/envoy-gateway 8080:80

# Test the endpoint with correct API key
curl -X POST http://localhost:8080/garage \
  -H "x-api-key: YOUR-API-KEY-HERE"

# Expected response:
# {"status":"success","message":"Garage door triggered"}

# Test that wrong API key is rejected
curl -X POST http://localhost:8080/garage \
  -H "x-api-key: wrong-key"

# Expected: 401 Unauthorized
```

## Step 8: Set Up iOS Shortcut

1. Open **Shortcuts** app on iOS
2. Tap **+** to create new shortcut
3. Add **"Get Contents of URL"** action
4. Configure:
   - URL: `https://your-cluster-domain/garage`
   - Method: **POST**
   - Headers: Add header
     - Key: `x-api-key`
     - Value: `<your-api-key>`
5. (Optional) Add **"Show Notification"** action to confirm success
6. Name it "Open Garage" or "Öffne Garage"
7. Tap Done

### Make it Easy to Access:

- **Siri**: Say "Hey Siri, Open Garage"
- **Widget**: Add to Home Screen or Lock Screen widget
- **Home Screen**: Add shortcut icon

## Step 9: Expose Externally (Production)

The Envoy gateway is already configured as a LoadBalancer service. Get the external IP:

```bash
kubectl get svc envoy-gateway
```

This will show the external IP or hostname:
```
NAME             TYPE           EXTERNAL-IP       PORT(S)
envoy-gateway    LoadBalancer   <your-ip>         80:xxxxx/TCP
```

**Next steps:**
1. Point your DNS record (e.g., `garage.your-domain.com`) to the EXTERNAL-IP
2. Configure TLS/HTTPS at your load balancer or use cert-manager
3. Update your iOS Shortcut URL to use your domain: `https://garage.your-domain.com/garage`

**Note:** If your cluster doesn't support LoadBalancer (like minikube), change the service type in `k8s/envoy-config.yaml` to `NodePort` and access via node IP:port.

## Troubleshooting

### Pod won't start
```bash
kubectl describe pod -l app=garage-mqtt-bridge
kubectl logs -l app=garage-mqtt-bridge
```

### Connection refused
- Check if MQTT broker is reachable from cluster
- Verify certificates are valid
- Check firewall rules

### Authentication fails
- Double-check API key matches in secret and iOS Shortcut
- Test without Envoy first (port-forward directly to service)

### iOS Shortcut doesn't work
- Test with curl first
- Ensure cluster endpoint is accessible from internet
- Check SSL/TLS if using HTTPS
- Verify API key header is exactly: `x-api-key`

## Next Steps

- [ ] Set up monitoring/alerting
- [ ] Configure cert-manager for certificate rotation
- [ ] Add rate limiting to prevent abuse
- [ ] Set up proper DNS and TLS certificates
- [ ] Configure network policies for security

## Need Help?

Check the full [README.md](README.md) for detailed documentation and troubleshooting.
