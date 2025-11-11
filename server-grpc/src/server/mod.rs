pub mod config;
use crate::generated::carbon_admin_operations_server::CarbonAdminOperations;
use crate::generated::carbon_cache_operations_server::CarbonCacheOperations;
use crate::generated::{
    CreateCacheRequest, CreateCacheResponse, DeleteRequest, DeleteResponse, DescribeCacheRequest,
    DescribeCacheResponse, DropCacheRequest, DropCacheResponse, GetRequest, GetResponse,
    ListCachesRequest, ListCachesResponse, PutRequest, PutResponse,
};
use carbon::domain::{CacheConfig, EvictionPolicy};
use carbon::planes::control::CacheManager;
use carbon::planes::data::operation::CacheOperations;
use carbon::planes::data::CacheOperationsService;
use carbon::ports::AdminOperations;
use log::info;
use tonic::{Request, Response, Status};

#[derive(Debug)]
pub struct CarbonServer {
    cache_manager: CacheManager<Vec<u8>, Vec<u8>>,
    cache_operations: CacheOperationsService<Vec<u8>, Vec<u8>>,
}

impl CarbonServer {
    pub fn new(cache_manager: CacheManager<Vec<u8>, Vec<u8>>) -> Self {
        let cache_operations = CacheOperationsService::new(cache_manager.clone());
        Self {
            cache_manager,
            cache_operations,
        }
    }
}

impl Default for CarbonServer {
    fn default() -> Self {
        Self::new(CacheManager::new())
    }
}

#[tonic::async_trait]
impl CarbonCacheOperations for CarbonServer {
    async fn put(&self, request: Request<PutRequest>) -> Result<Response<PutResponse>, Status> {
        let req = request.into_inner();
        info!("PUT: cache={}", req.cache);

        // Convert protobuf -> domain
        let ttl = if req.ttl_ms > 0 {
            Some(shared::TtlMs(req.ttl_ms))
        } else {
            None
        };

        // Call application service (hexagon)
        match self
            .cache_operations
            .put(&req.cache, req.key, req.value, ttl)
            .await
        {
            Ok(_) => Ok(Response::new(PutResponse { ok: true })),
            Err(shared::Error::CacheNotFound(name)) => {
                Err(Status::not_found(format!("Cache '{}' not found", name)))
            }
            Err(e) => Err(Status::internal(format!("Put failed: {}", e))),
        }
    }

    async fn get(&self, request: Request<GetRequest>) -> Result<Response<GetResponse>, Status> {
        let req = request.into_inner();
        info!("GET: cache={}", req.cache);

        // Call application service (hexagon)
        match self.cache_operations.get(&req.cache, &req.key).await {
            Ok(result) => {
                // Convert domain -> protobuf
                Ok(Response::new(GetResponse {
                    found: result.found,
                    value: result.message,
                    ttl_ms_remaining: 0, // TODO: Implement TTL tracking
                }))
            }
            Err(shared::Error::NotFound) => Ok(Response::new(GetResponse {
                found: false,
                value: vec![],
                ttl_ms_remaining: 0,
            })),
            Err(shared::Error::CacheNotFound(name)) => {
                Err(Status::not_found(format!("Cache '{}' not found", name)))
            }
            Err(e) => Err(Status::internal(format!("Get failed: {}", e))),
        }
    }

    async fn delete(
        &self,
        request: Request<DeleteRequest>,
    ) -> std::result::Result<Response<DeleteResponse>, Status> {
        let req = request.into_inner();
        info!("DELETE: cache={}", req.cache);

        // Call application service (hexagon)
        match self.cache_operations.delete(&req.cache, &req.key).await {
            Ok(result) => {
                // Convert domain -> protobuf
                Ok(Response::new(DeleteResponse {
                    deleted: result.deleted,
                }))
            }
            Err(shared::Error::CacheNotFound(name)) => {
                Err(Status::not_found(format!("Cache '{}' not found", name)))
            }
            Err(e) => Err(Status::internal(format!("Delete failed: {}", e))),
        }
    }
}

#[tonic::async_trait]
impl CarbonAdminOperations for CarbonServer {
    async fn create_cache(
        &self,
        request: Request<CreateCacheRequest>,
    ) -> Result<Response<CreateCacheResponse>, Status> {
        info!("Got a CREATE_CACHE request: {:?}", request);
        let proto_config = request.into_inner().config.unwrap();

        // Convert protobuf config to domain config
        let cache_config = CacheConfig::new(
            proto_config.name.clone(),
            proto_config.mem_bytes,
            if proto_config.disk_path.is_empty() {
                None
            } else {
                Some(proto_config.disk_path)
            },
            proto_config.shards,
            EvictionPolicy::try_from(proto_config.policy).unwrap_or(EvictionPolicy::TinyLfu),
            Some(proto_config.default_ttl_ms),
            Some(proto_config.max_value_bytes),
        );

        // Create a Foyer cache instance (adapter layer)
        let foyer_cache = storage_engine::FoyerCache::new(cache_config.mem_bytes as usize);

        // Register it with the cache manager
        if let Err(e) = self
            .cache_manager
            .register_cache(proto_config.name.clone(), std::sync::Arc::new(foyer_cache))
            .await
        {
            return Err(Status::internal(format!("Failed to register cache: {}", e)));
        }

        // Call the admin operation
        match self.cache_manager.create_cache(cache_config).await {
            Ok(result) => {
                let response = CreateCacheResponse {
                    created: result.created,
                    message: result.message,
                };
                Ok(Response::new(response))
            }
            Err(e) => Err(Status::internal(format!("Failed to create cache: {}", e))),
        }
    }

    async fn drop_cache(
        &self,
        request: Request<DropCacheRequest>,
    ) -> Result<Response<DropCacheResponse>, Status> {
        info!("Got a DROP_CACHE request: {:?}", request);
        let response = DropCacheResponse::default();
        Ok(Response::new(response))
    }

    async fn list_caches(
        &self,
        request: Request<ListCachesRequest>,
    ) -> Result<Response<ListCachesResponse>, Status> {
        info!("Got a LIST_CACHES request: {:?}", request);
        let response = ListCachesResponse::default();
        Ok(Response::new(response))
    }

    async fn describe_cache(
        &self,
        request: Request<DescribeCacheRequest>,
    ) -> Result<Response<DescribeCacheResponse>, Status> {
        info!("Got a DESCRIBE_CACHE request: {:?}", request);
        let response = DescribeCacheResponse::default();
        Ok(Response::new(response))
    }
}
