use std::sync::Arc;
use futures::{StreamExt};
use kube::runtime::{watcher::Config};
use kube::Resource;
use kube::ResourceExt;
use kube::{client::Client, runtime::controller::Action, runtime::Controller, Api};
use k8s_openapi::api::core::v1::{Node};
use tokio::time::Duration;
use crate::crd::HealthCheck;
use futures::future::FutureExt;
use kube::api::ListParams;
use std::collections::BTreeMap;
use crate::database::{
    get_by_node_ip_nbcfg,
    get_db_state,
};

pub mod crd;
mod actions;
mod hcapi;
mod database;

#[tokio::main]
async fn main() {
    let kubernetes_client: Client = Client::try_default()
        .await
        .expect("Expected a valid KUBECONFIG environment variable.");

    let node_api: Api<Node> = Api::all(kubernetes_client.clone());
    let context: Arc<ContextData> = Arc::new(ContextData::new(kubernetes_client.clone()));
    let nodes = node_api.list(&Default::default()).await.unwrap();
    println!("Active nodes at start: {}", nodes.items.len());
    let mut result = true;
    for node in nodes.items {
        if let Some(annotations) = &node.metadata.annotations {
            if !annotations.is_empty() {
                let annotation_key = "test.example.com/seen_by_operator";
                if let Some(annotation_value) = annotations.get(annotation_key) {
                    result = false;
                }
            }
        }
    }

    Controller::new(node_api.clone(), Config::default())
        .graceful_shutdown_on(tokio::signal::ctrl_c().map(|_| ()))
        .run(reconcile, on_error, context)
        .for_each(|reconciliation_result| async move {
            match reconciliation_result {
                Ok(node_resource) => {
                    println!("Reconciliation successful. Resource: {:?}", node_resource);
                    //println!("Reconciliation successful.");
                }
                Err(reconciliation_err) => {
                    eprintln!("Reconciliation error: {:?}", reconciliation_err)
                }
            }
        })
        .await;
}

struct ContextData {
    client: Client,
}

impl ContextData {
    pub fn new(client: Client) -> Self {
        ContextData { client }
    }
}

enum HealthCheckAction {
    Create,
    Delete,
    NoOp,
}

pub async fn get_lke_id(node: Arc<Node>) -> String {
    //CLean this up from an error detection and handling
    let annotations: Option<&BTreeMap<String, String>> = node.metadata.annotations.as_ref();
    let annotation_key = "cluster.x-k8s.io/cluster-name";
    let Some(annotation_value) = annotations.expect("").get(annotation_key) else { todo!() };

    annotation_value.to_string()

}

async fn reconcile(node: Arc<Node>, context: Arc<ContextData>) -> Result<Action, Error> {
    //Changed namespace logic based on customer requirements. Maybe list valid namespaces as vec in CRD definitions? 
    let client: Client = context.client.clone();
    let hcapi: Api<HealthCheck> = Api::namespaced(client.clone(), "default");
    let name = node.metadata.name.clone().expect("Cannot get node name.").to_string();
    let cluster_name = get_lke_id(node.clone()).await;
    let lp = ListParams::default();
    let healthchecks = hcapi.list(&lp).await.unwrap();
    for hclist in healthchecks.items.clone() {
        let hc = hcapi.get(&hclist.metadata.name.expect("HC lookup issue")).await.unwrap();
        println!("{}-{}-{}", hc.spec.serv_namespace, hc.spec.timeout, hc.spec.port);
    }

    match determine_action(&node) {
        HealthCheckAction::Create => {
            //let hc = hcapi.get("hc").await.unwrap();
            //println!("Vector {:?}", &healthchecks.items);
            for hclist in &healthchecks.items {
                let hc = hcapi.get(&hclist.metadata.name.clone().expect("HC lookup issue")).await.unwrap();
                //println!("{:#?}", hc);
                let srv_namespace = hc.spec.serv_namespace;
                let timeout = hc.spec.timeout;
                let port = hc.spec.port;
                let seen_before = actions::check_if_seen_before(client.clone(), &name).await;
                println!("{:?}", seen_before);
                //actions::mark_as_seen(client.clone(), &name).await?;
                actions::check_pod(client.clone(), &name, &srv_namespace).await;
                let hcpod_ip = actions::get_hc_pod_ip(client.clone(), &name, &srv_namespace, port.clone()).await;
                println!("hcppod_ip: {:?}", hcpod_ip);
                let mut result = false;
                let null_ip = "0.0.0.0".to_string();

                if !hcpod_ip.contains(&null_ip) {
                    for ip in hcpod_ip {
                        result = actions::check_port(ip.clone(), port, timeout).await;
                        println!("Port check passed status: {:?}", result);

                        let state = actions::get_state(port.clone(), ip.clone(), &cluster_name).await;
                        println!("{:?}", state);
                        //let state = actions::get_state(port.clone(), ip.clone(), &cluster_name).await;
                        if result == true {
                            let _ = actions::add_to_nb(client.clone(), &name, port.clone(), ip.clone(), &cluster_name).await;
                            println!("reachable");
                        } else if result == false && state.1 != "drain" {
                            let _ = actions::remove_from_nb(client.clone(), &name, port.clone(), ip.clone(), &cluster_name).await;
                            println!("Node {:?} removed from NodeBalancer - unreachable", &name);
                        }
                    }
                } else {
                    return Ok(Action::requeue(Duration::from_secs(10)))
                }
            }
            return Ok(Action::requeue(Duration::from_secs(10)))
        }
        HealthCheckAction::Delete => {
            Ok(Action::await_change())
        }
        HealthCheckAction::NoOp => Ok(Action::requeue(Duration::from_secs(10))),
    }
}

fn determine_action(node: &Node) -> HealthCheckAction {
    if node.meta().deletion_timestamp.is_some() {
        HealthCheckAction::Delete
    } else if node 
        .meta()
        .finalizers
        .as_ref()
        .map_or(true, |finalizers| finalizers.is_empty())
    {
        HealthCheckAction::Create
    } else {
        HealthCheckAction::NoOp
    }
}

fn on_error(node: Arc<Node>, error: &Error, _context: Arc<ContextData>) -> Action {
    eprintln!("Reconciliation error:\n{:?}.\n{:?}", error, node);
    Action::requeue(Duration::from_secs(5))
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Kubernetes reported error: {source}")]
    KubeError {
        #[from]
        source: kube::Error,
    },
    #[error("Invalid HealthCheck CRD: {0}")]
    UserInputError(String),
}
