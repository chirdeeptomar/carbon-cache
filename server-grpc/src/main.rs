use log::{error, info};

use std::net::ToSocketAddrs;

use tonic::transport::Server;

use crate::generated::carbon_data_service_server::CarbonDataServiceServer;
use crate::server::config::{get_scheme_grpc, get_server_host, get_server_port};

mod generated;
mod server;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    info!("Starting Carbon gRPC Server...");
    let address = format!("{}:{}", get_server_host(), get_server_port());

    let server = server::CarbonServer::default();

    let grpc_server = Server::builder()
        .add_service(CarbonDataServiceServer::new(server))
        .serve(address.to_socket_addrs().unwrap().next().unwrap());

    info!("Server Listening on: {}://{}", get_scheme_grpc(), address);

    if let Err(e) = grpc_server.await {
        error!("server error: {}", e);
    }

    Ok(())
}
