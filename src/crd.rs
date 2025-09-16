use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(CustomResource, Serialize, Deserialize, Debug, PartialEq, Clone, JsonSchema)]
#[kube(
    group = "example.com",
    version = "v1",
    kind = "HealthCheck",
    plural = "healthchecks",
    derive = "PartialEq",
    namespaced
)]
pub struct HealthCheckSpec {
    pub timeout: u64,
    pub port: i32,
    pub serv_namespace: String,
}
