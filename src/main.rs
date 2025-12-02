use actix_web::{web, App, HttpResponse, HttpServer, Responder};
use log::{error, info};
use native_tls::{Certificate, Identity, TlsConnector};
use rumqttc::{AsyncClient, MqttOptions, QoS, Transport};
use std::fs;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

struct AppState {
    mqtt_client: Arc<Mutex<AsyncClient>>,
}

async fn trigger_garage(data: web::Data<AppState>) -> impl Responder {
    info!("Received garage door trigger request");

    // Topic should be configured via environment variable in production
    let topic = std::env::var("MQTT_TOPIC").unwrap_or_else(|_| "garage/trigger".to_string());
    let payload = std::env::var("MQTT_PAYLOAD").unwrap_or_else(|_| "1".to_string());

    let client = data.mqtt_client.lock().await;
    match client.publish(
        &topic,
        QoS::AtLeastOnce,
        false,
        payload.as_bytes(),
    ).await {
        Ok(_) => {
            info!("Successfully published MQTT message");
            HttpResponse::Ok().json(serde_json::json!({
                "status": "success",
                "message": "Garage door triggered"
            }))
        }
        Err(e) => {
            error!("Failed to publish MQTT message: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({
                "status": "error",
                "message": format!("Failed to trigger garage door: {}", e)
            }))
        }
    }
}

async fn health_check() -> impl Responder {
    HttpResponse::Ok().json(serde_json::json!({
        "status": "healthy"
    }))
}

fn load_tls_config(
    ca_path: &str,
    cert_path: &str,
    key_path: &str,
) -> Result<TlsConnector, Box<dyn std::error::Error>> {
    // Load CA certificate
    let ca_cert_pem = fs::read(ca_path)?;
    let ca_cert = Certificate::from_pem(&ca_cert_pem)?;

    // Load client certificate and key as PKCS#12/PFX
    // native-tls requires Identity from PKCS#12, so we need to convert PEM to PKCS#12
    let cert_pem = fs::read(cert_path)?;
    let key_pem = fs::read(key_path)?;

    // Create identity from PEM certificate and key
    let identity = Identity::from_pkcs8(&cert_pem, &key_pem)?;

    // Build TLS connector
    let connector = TlsConnector::builder()
        .add_root_certificate(ca_cert)
        .identity(identity)
        .build()?;

    Ok(connector)
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));

    // Read configuration from environment variables
    let mqtt_host = std::env::var("MQTT_HOST").unwrap_or_else(|_| "mqtt.example.com".to_string());
    let mqtt_port: u16 = std::env::var("MQTT_PORT")
        .unwrap_or_else(|_| "8883".to_string())
        .parse()
        .expect("Invalid MQTT_PORT");
    let ca_path = std::env::var("CA_CERT_PATH").unwrap_or_else(|_| "/certs/ca.crt".to_string());
    let cert_path = std::env::var("CLIENT_CERT_PATH").unwrap_or_else(|_| "/certs/client.crt".to_string());
    let key_path = std::env::var("CLIENT_KEY_PATH").unwrap_or_else(|_| "/certs/client.key".to_string());
    let http_port: u16 = std::env::var("HTTP_PORT")
        .unwrap_or_else(|_| "8080".to_string())
        .parse()
        .expect("Invalid HTTP_PORT");

    info!("Initializing MQTT client...");
    info!("MQTT Broker: {}:{}", mqtt_host, mqtt_port);

    // Set up MQTT options
    let mut mqtt_options = MqttOptions::new("garage-mqtt-bridge", mqtt_host, mqtt_port);
    mqtt_options.set_keep_alive(Duration::from_secs(30));

    // Load TLS configuration
    let tls_connector = load_tls_config(&ca_path, &cert_path, &key_path)
        .expect("Failed to load TLS certificates");

    mqtt_options.set_transport(Transport::tls_with_config(tls_connector.into()));

    // Create MQTT client
    let (client, mut eventloop) = AsyncClient::new(mqtt_options, 10);
    let client = Arc::new(Mutex::new(client));

    // Spawn a task to handle the MQTT connection
    tokio::spawn(async move {
        info!("Starting MQTT event loop...");
        loop {
            match eventloop.poll().await {
                Ok(notification) => {
                    info!("MQTT notification: {:?}", notification);
                }
                Err(e) => {
                    error!("MQTT connection error: {}. Retrying...", e);
                    tokio::time::sleep(Duration::from_secs(5)).await;
                }
            }
        }
    });

    // Allow MQTT connection to establish
    tokio::time::sleep(Duration::from_secs(2)).await;

    info!("Starting HTTP server on 0.0.0.0:{}...", http_port);

    // Create application state
    let app_state = web::Data::new(AppState {
        mqtt_client: client,
    });

    // Start HTTP server
    HttpServer::new(move || {
        App::new()
            .app_data(app_state.clone())
            .route("/garage", web::post().to(trigger_garage))
            .route("/health", web::get().to(health_check))
    })
    .bind(("0.0.0.0", http_port))?
    .run()
    .await
}
