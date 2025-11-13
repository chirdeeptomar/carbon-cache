mod generated;
mod server;

use log::{error, info};
use std::net::ToSocketAddrs;
use std::sync::Arc;
use tonic::transport::Server;
use tonic_reflection::server::Builder as ReflectionBuilder;

use crate::generated::carbon_admin_operations_server::CarbonAdminOperationsServer;
use crate::generated::carbon_cache_operations_server::CarbonCacheOperationsServer;
use crate::server::config::{get_scheme_grpc, get_server_host, get_server_port};

const FILE_DESCRIPTOR_SET: &[u8] = tonic::include_file_descriptor_set!("service_descriptor");

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    info!("Starting Carbon gRPC Server...");
    let address = format!("{}:{}", get_server_host(), get_server_port());

    let server = Arc::new(server::CarbonServer::default());

    // Build reflection service
    let reflection_service = ReflectionBuilder::configure()
        .register_encoded_file_descriptor_set(FILE_DESCRIPTOR_SET)
        .build_v1()?;

    let grpc_server = Server::builder()
        .add_service(CarbonCacheOperationsServer::from_arc(server.clone()))
        .add_service(CarbonAdminOperationsServer::from_arc(server))
        .add_service(reflection_service)
        .serve(address.to_socket_addrs().unwrap().next().unwrap());

    info!("Server Listening on: {}://{}", get_scheme_grpc(), address);

    if let Err(e) = grpc_server.await {
        error!("server error: {}", e);
    }

    Ok(())
}
