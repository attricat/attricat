//! A local HTTPS server for brokered-egress tests. Its certificate is issued
//! by the fixture CA in `tests/fixtures/mock-tls` for `*.mock.test`; tests
//! route those names here with the development egress override.

use std::{net::SocketAddr, path::PathBuf, sync::Arc};

use axum::Router;
use hyper_util::{
    rt::{TokioExecutor, TokioIo},
    server::conn::auto::Builder,
    service::TowerToHyperService,
};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, pem::PemObject};
use tokio::net::TcpListener;
use tokio_rustls::TlsAcceptor;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/mock-tls")
        .join(name)
}

/// The PEM of the CA that issued the mock's certificate.
pub fn mock_ca_pem() -> Vec<u8> {
    std::fs::read(fixture("ca.pem")).unwrap()
}

/// Serves `router` over TLS on a loopback port and returns its address.
pub async fn serve_tls(router: Router) -> SocketAddr {
    let certificates = CertificateDer::pem_file_iter(fixture("server.pem"))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    let key = PrivateKeyDer::from_pem_file(fixture("server.key")).unwrap();
    let config = rustls::ServerConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .unwrap()
    .with_no_client_auth()
    .with_single_cert(certificates, key)
    .unwrap();
    let acceptor = TlsAcceptor::from(Arc::new(config));
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move {
        loop {
            let Ok((stream, _)) = listener.accept().await else {
                return;
            };
            let acceptor = acceptor.clone();
            let router = router.clone();
            tokio::spawn(async move {
                let Ok(stream) = acceptor.accept(stream).await else {
                    return;
                };
                let _ = Builder::new(TokioExecutor::new())
                    .serve_connection(TokioIo::new(stream), TowerToHyperService::new(router))
                    .await;
            });
        }
    });
    address
}
