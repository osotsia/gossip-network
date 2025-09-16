//! src/transport/tls.rs
//!
//! Manages the configuration of TLS for QUIC using a private PKI.

use crate::error::{Error, Result};
use quinn::{ClientConfig, ServerConfig};
use std::{fs, sync::Arc};

/// Configures TLS for the client and server using a shared private CA.
/// Expects `ca.cert`, `node.cert`, and `node.key` files in the `certs/` directory.
pub fn configure_tls() -> Result<(ServerConfig, ClientConfig)> {
    // Load the certificate authority.
    let ca_cert_der = fs::read("certs/ca.cert").map_err(|e| {
        Error::TlsConfig(format!("Failed to read CA certificate (certs/ca.cert): {}", e))
    })?;
    let ca_cert = rustls::p_k_i_types::CertificateDer::from(ca_cert_der);

    // Configure the client to trust the CA.
    let mut root_store = rustls::RootCertStore::empty();
    root_store.add(ca_cert.clone()).map_err(|e| {
        Error::TlsConfig(format!("Failed to add CA to root store: {}", e))
    })?;
    let mut client_config = ClientConfig::with_root_certificates(root_store)?;
    client_config.alpn_protocols = vec![b"gossip/1.0".to_vec()];

    // Configure the server with its own certificate and private key.
    let cert_chain_der = fs::read("certs/node.cert").map_err(|e| {
        Error::TlsConfig(format!("Failed to read node certificate (certs/node.cert): {}", e))
    })?;
    let key_der = fs::read("certs/node.key").map_err(|e| {
        Error::TlsConfig(format!("Failed to read node private key (certs/node.key): {}", e))
    })?;
    let cert_chain = vec![rustls::p_k_i_types::CertificateDer::from(cert_chain_der)];
    let key = rustls::p_k_i_types::PrivatePkcs8KeyDer::from(key_der).into();

    let mut server_config = ServerConfig::with_single_cert(cert_chain, key)
        .map_err(|e| Error::TlsConfig(format!("Failed to create server config: {}", e)))?;
    server_config.alpn_protocols = vec![b"gossip/1.0".to_vec()];

    let transport_config = Arc::get_mut(&mut server_config.transport).unwrap();
    transport_config.keep_alive_interval(Some(std::time::Duration::from_secs(10)));

    Ok((server_config, client_config))
}

/*
--------------------------------------------------------------------------------
-- HOW TO GENERATE CERTIFICATES FOR THE PRIVATE PKI
--------------------------------------------------------------------------------
This setup requires a private Public Key Infrastructure (PKI) to ensure that
only authorized nodes can connect to each other. A simple tool like `minica`
can be used for this.

The following steps must be completed before running the application.

1. Install `minica` (requires Go):
   go install github.com/jsha/minica@latest

2. Create a directory for certificates at the project root:
   mkdir certs
   cd certs

3. Generate the Certificate Authority (CA) and a certificate for "localhost".
   All our nodes will use the "localhost" server name for TLS SNI.
   minica --domains localhost

   This will create:
     - `minica.pem` and `minica.key` (The CA)
     - `localhost/cert.pem` and `localhost/key.pem` (The node's certificate)

4. Convert the PEM files to the DER format that rustls expects:
   openssl x509 -outform der -in minica.pem -out ca.cert
   openssl x509 -outform der -in localhost/cert.pem -out node.cert
   openssl pkcs8 -topk8 -nocrypt -outform der -in localhost/key.pem -out node.key

5. Verify the `certs/` directory. It should now contain:
   - ca.cert
   - node.cert
   - node.key

For this demonstration project, all nodes in the network will share these same
three files. In a real-world system, each node would have its own unique
`node.cert` and `node.key` files, all signed by the same central `ca.cert`.
--------------------------------------------------------------------------------
*/