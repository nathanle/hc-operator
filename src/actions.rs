use crate::crd::HealthCheck;
use k8s_openapi::api::core::v1::NodeAddress;
use kube::api::{ListParams, Patch, PatchParams};
use kube::{Api, Client, Error};
use serde_json::{from_value, json, Value};
use std::time::Duration;
use k8s_openapi::api::core::v1::{Node, Pod};
use port_check::*;                                                                                                                                                                                 
//use std::net::*;
use std::collections::BTreeMap;
use serde::{Serialize, Deserialize};
//use serde_json_path::JsonPath;
//use kube::api::ObjectMeta;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::{ObjectMeta, Time, ManagedFieldsEntry, OwnerReference};
use k8s_openapi::api::core::v1::{ PodSpec, PodStatus };
use tokio_postgres::Row;
use crate::hcapi;
use crate::database::{
    get_by_node_ip_nbcfg,
    update_state,
    get_db_state,
};


//mod database;

#[derive(Serialize, Deserialize, Debug)]
struct NodeMetadataPatch {
    labels: BTreeMap<String, String>,
}

// Define a struct to represent the overall Pod patch
#[derive(Serialize, Deserialize, Debug)]
struct NodePatch {
    metadata: NodeMetadataPatch,
}

fn get_private_address(node: &Node) -> Option<String> {
    if let Some(addresses) = node.status.as_ref().and_then(|s| s.addresses.as_ref()) {
        for address in addresses {
            if address.type_ == "InternalIP" {
                return Some(address.address.clone());
            }
        }
    }
    None
}

pub async fn get_hc_pod_ip(client: Client, target_node_name: &String, ns: &str, hcport: i32) -> Vec<String> {

    #[derive(Serialize, Deserialize, Debug)]
    struct ContainerPort {
        container_port: u32,
    }

    #[derive(Serialize, Deserialize, Debug)]
    struct ZeroPods {
        //Pod: ObjectMeta,
    }

    #[derive(Serialize, Deserialize, Debug)]
    struct Pods {
        apiVersion: String,
        kind: String,
        metadata: ObjectMeta,
        spec: PodSpec,
        status: PodStatus,
        //Pod: MetaData,
    }

    #[derive(Serialize, Deserialize, Debug)]
    struct MetaData {
        metadata: ObjectMeta,
        spec: PodSpec,
        status: PodStatus,
    }

    let mut ip_vector: Vec<String> = Vec::new();
    let mut t: i32 = 0;
    //let mut hcpod_ip: String = "0.0.0.0".to_string();
    let pods: Api<Pod> = Api::namespaced(client, ns);
    let pod_list = pods.list(&ListParams::default()).await.unwrap();
    let filtered_pods: Vec<Pod> = pod_list
        .items
        .into_iter()
        .filter(|p| {
            if let Some(spec) = &p.spec {
                spec.node_name.as_deref() == Some(&target_node_name)
            } else {
                false
            }
        })
        .filter(|p| {
            if !p.metadata.name.as_deref().unwrap().contains("node-health-check-operator") {
                true
            } else {
                false
            }
        })
        .collect();
    if filtered_pods.len() == 0 {
        let filtered_pods_results = serde_json::to_string(&filtered_pods).unwrap();
        let my_struct: ZeroPods = serde_json::from_str(&filtered_pods_results).expect("ZeroPods Failed");
        ip_vector.push("0.0.0.0".to_string());
        
    }
    if filtered_pods.len() > 0 {
        for f in filtered_pods {
            let filtered_pods_results = serde_json::to_value(f).unwrap();
            let my_struct: Pods = serde_json::from_value(filtered_pods_results).expect("Pod Failed");
            //This is where the bug is when new pods start. Panic sometimes occurs
            let result = my_struct.status.pod_ip.clone();
            if result != None {
                ip_vector.push(result.clone().expect("IP string conversion failed").to_string());
            }
                //Ok(val) if val == s.unwrap() => ip_vector.push(s.to_string()),
                //Err(val) if val == e.unwrap() => ip_vector.push("0.0.0.0".to_string()),

        }
    }
    return ip_vector
}

pub async fn check_port(ip_address: String, port_number: i32, check_timeout: u64) -> bool {                                                                                                                                        
     let addr = format!("{}:{}", ip_address.to_string(), port_number);                                                                                                                              
     is_port_reachable_with_timeout(addr, Duration::from_secs(check_timeout))                                                                                                                                                    

     //true
     
 }

pub async fn check_if_seen_before(client: Client, name: &str) -> bool {
    let api: Api<Node> = Api::all(client);
    let mut node = api.get(&name).await.unwrap();
    let annotations: Option<&BTreeMap<String, String>> = node.metadata.annotations.as_ref();
    let mut result = false;
    if let Some(annotations) = annotations {
        let annotation_key = "";
        if let Some(annotation_value) = annotations.get(annotation_key) {
            result = true;
        } else {
            result = false;
        }
    } else {
        result = false;
    }
    result

}

pub async fn remove_from_nb(client: Client, name: &str, port: i32, podip: String, clustername: &String) {
    let api: Api<Node> = Api::all(client);
    let node = api.get(&name).await.unwrap();
    let private_ip = get_private_address(&node);
    let dbresp = get_by_node_ip_nbcfg(&private_ip.unwrap(), &port).await;
    let response = dbresp.unwrap();
    let mode = "drain";
    let hcstatus = "drain";
    for row in response {
        let nodeid: i32 = row.get(0);
        let cfgid: i32 = row.get(3);
        let nbid: i32 = row.get(4);
        println!("REMOVE: Node ID {} = Config ID {} = NodeBalancer ID {} = Port {}", nodeid, cfgid, nbid, port);
        let _ = hcapi::change_node_mode(&nbid, &cfgid, &nodeid, (&mode).to_string()).await;
        let _ = update_state(nbid, cfgid, nodeid, &podip, port, (&mode).to_string(), (&hcstatus).to_string(), clustername).await;

    }

}

pub async fn get_state(port: i32, podip: String, clustername: &String) -> (String, String) {
    let result = get_db_state(port, podip, &clustername).await;
    let mut lastmode: String = String::new();
    let mut current: String = String::new();
    for row in result.unwrap() {

        lastmode = row.get(5);
        current = row.get(6);

    }

    (lastmode, current)
}

pub async fn init_state(client: Client, name: &str, port: i32, podip: String, clustername: &String) {
    let api: Api<Node> = Api::all(client);
    let node = api.get(&name).await.unwrap();
    let private_ip = get_private_address(&node);
    let dbresp = get_by_node_ip_nbcfg(&private_ip.unwrap(), &port).await;
    let response = dbresp.unwrap();
    let mode = "none";
    let hcstatus = "none";
    for row in response {
        let nodeid: i32 = row.get(0);
        let cfgid: i32 = row.get(3);
        let nbid: i32 = row.get(4);
        let _ = update_state(nbid, cfgid, nodeid, &podip, port, (&mode).to_string(), (&hcstatus).to_string(), clustername).await;
    }
}

pub async fn add_to_nb(client: Client, name: &str, port: i32, podip: String, clustername: &String) {
    let api: Api<Node> = Api::all(client);
    let node = api.get(&name).await.unwrap();
    let private_ip = get_private_address(&node);
    let dbresp = get_by_node_ip_nbcfg(&private_ip.unwrap(), &port).await;
    let response = dbresp.unwrap();
    let mode = "accept";
    let hcstatus = "accept";
    for row in response {
        let nodeid: i32 = row.get(0);
        let cfgid: i32 = row.get(3);
        let nbid: i32 = row.get(4);
        println!("ADD: Node ID {} = Config ID {} = NodeBalancer ID {} = Port {}", nodeid, cfgid, nbid, port);
        let _ = hcapi::change_node_mode(&nbid, &cfgid, &nodeid, (&mode).to_string()).await;
        let _ = update_state(nbid, cfgid, nodeid, &podip, port, (&mode).to_string(), (&hcstatus).to_string(), clustername).await;
    }
}
