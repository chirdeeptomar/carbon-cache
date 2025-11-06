pub mod config;

use crate::generated;
use log::info;
use tonic::{Request, Response, Status};

use generated::carbon_data_service_server::CarbonDataService;
use generated::{DeleteRequest, DeleteResponse, GetRequest, GetResponse, PutRequest, PutResponse};

#[derive(Debug, Default)]
pub struct CarbonServer {}

#[tonic::async_trait]
impl CarbonDataService for CarbonServer {
    async fn put(&self, request: Request<PutRequest>) -> Result<Response<PutResponse>, Status> {
        info!("Got a PUT request: {:?}", request);
        let response = PutResponse { ok: true };
        Ok(Response::new(response))
    }

    async fn get(&self, request: Request<GetRequest>) -> Result<Response<GetResponse>, Status> {
        info!("Got a GET request: {:?}", request);
        let response = GetResponse {
            value: "some_value".into(),
            found: true,
            ttl_ms_remaining: 1,
        };
        Ok(Response::new(response))
    }

    async fn delete(
        &self,
        request: Request<DeleteRequest>,
    ) -> std::result::Result<Response<DeleteResponse>, Status> {
        info!("Got a DELETE request: {:?}", request);
        let response = DeleteResponse { deleted: true };
        Ok(Response::new(response))
    }
}
