use crate::crd::HealthCheck;
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

pub async fn check_pod(client: Client, target_node_name: &String, namespace: &str) {
    #[derive(Serialize, Deserialize, Debug)]
    struct Pods {
        apiVersion: String,
        kind: String,
        metadata: ObjectMeta,
        spec: PodSpec,
        status: PodStatus,
        //Pod: MetaData,
    }

    let mut t: i32 = 0;
    let mut s: String = "".to_string();
    let pods: Api<Pod> = Api::namespaced(client, namespace);
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
            if !p.metadata.name.as_ref().unwrap().contains("node-health-check-operator") {
                true
            } else {
                false
            }
        })
        .collect();
    println!(
        "\nFound {} pods on node {} in namespace {}",
        filtered_pods.len(),
        target_node_name,
        namespace
    );
    //println!("{:#?}", filtered_pods);
    for p in filtered_pods {
        println!("  - {}", p.metadata.name.as_ref().unwrap());
           if let Some(spec) = p.spec {
            //println!("{:#?}", spec.containers);
            for container in spec.containers {
                if let Some(ports) = container.ports {
                    for port in ports {
                        println!("  Container: {}, Port: {}", container.name, port.container_port);
                        t = port.container_port;
                    }
                }
                if let Some(status) = &p.status {
                    if let Some(pod_ip) = &status.pod_ip {
                        println!("Pod IP {}", pod_ip);
                        s = pod_ip.to_string();
                    } else {
                        println!("Pod IP not yet assigned.");
                    }
                }
            }
        }
    }
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
        .collect();
    if filtered_pods.len() == 0 {
        //println!(
        //    "\nFound {} pods on node '{}': {:#?}",
        //    filtered_pods.len(),
        //    target_node_name,
        //    filtered_pods
        //);
        let filtered_pods_results = serde_json::to_string(&filtered_pods).unwrap();
        let my_struct: ZeroPods = serde_json::from_str(&filtered_pods_results).expect("ZeroPods Failed");
        println!("{:#?}", my_struct);
            ip_vector.push("0.0.0.0".to_string());
        
    }
    if filtered_pods.len() > 0 {
        //println!(
        //    "\nFound {} pods on node '{}'",
        //    filtered_pods.len(),
        //    target_node_name,
        //);
        for f in filtered_pods {
            let filtered_pods_results = serde_json::to_value(f).unwrap();
            //println!("{:#?}", filtered_pods_results);
            let my_struct: Pods = serde_json::from_value(filtered_pods_results).expect("Pod Failed");
            //println!("{:#?}", my_struct.status.pod_ip);
            ip_vector.push(my_struct.status.pod_ip.expect("IP string conversion failed").to_string());
        }
    }
    println!("{:?}", ip_vector);
    //return "0.0.0.0".to_string()
    return ip_vector
}

pub async fn check_port(ip_address: String, port_number: i32, check_timeout: u64) -> bool {                                                                                                                                        
     let addr = format!("{}:{}", ip_address.to_string(), port_number);                                                                                                                              
     println!("{}", addr);                                                                                                                                                                          
     //let free_port = free_local_port().unwrap();                                                                                                                                                    
     is_port_reachable_with_timeout(addr, Duration::from_secs(check_timeout))                                                                                                                                                    
 }

pub async fn check_if_seen_before(client: Client, name: &str) -> bool {
    let api: Api<Node> = Api::all(client);
    let mut node = api.get(&name).await.unwrap();
    let annotations: Option<&BTreeMap<String, String>> = node.metadata.annotations.as_ref();
    let mut result = false;
    if let Some(annotations) = annotations {
        let annotation_key = "test.example.com/seen_by_operator";
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

//pub async fn mark_as_seen(client: Client, name: &str) -> Result<Node, Error> {
    //let api: Api<Node> = Api::all(client);
    //let mut node = api.get(&name).await.unwrap();
    //let mut annotations = node.metadata.annotations.unwrap_or_default();
    //annotations.insert("test.example.com/seen_by_operator".to_string(), "true".to_string());
    //node.metadata.annotations = Some(annotations);
    //let patch_payload: Value = json!({
    //    "metadata": {
    //        "annotations": node.metadata.annotations
    //    }
    //});
    //let patch: Patch<&Value> = Patch::Merge(&patch_payload);
    //println!("Annotations updated for node: {}", name);
    //api.patch(name, &PatchParams::default(), &patch).await
//}

pub async fn remove_from_nb(client: Client, name: &str) -> Result<Node, Error> {
    let api: Api<Node> = Api::all(client);
    let mut node = api.get(&name).await.unwrap();
    let mut annotations = node.metadata.annotations.unwrap_or_default();
    annotations.insert("node.k8s.linode.com/exclude-from-nb".to_string(), "true".to_string());
    node.metadata.annotations = Some(annotations);
    let patch_payload: Value = json!({
        "metadata": {
            "annotations": node.metadata.annotations
        }
    });
    let patch: Patch<&Value> = Patch::Merge(&patch_payload);
    println!("Annotations updated for node: {} - Removed from NB", name);
    api.patch(name, &PatchParams::default(), &patch).await
    
}

pub async fn remove_from_nb_old(client: Client, name: &str) -> Result<Node, Error> {
    let api: Api<Node> = Api::all(client);
    let mut node = api.get(&name).await.unwrap();
    //let mut annotations = node.metadata.annotations.unwrap_or_default();
    let annotation_key = "node.k8s.linode.com/exclude-from-nb";

    let patch = json!([
        {
            "op": "remove",
            "path": format!("/metadata/annotations/{}", annotation_key.replace("~", "~0").replace("/", "~1"))
        }
    ]);
    println!("Annotations updated for node: {} - Added to NB", name);

    api.patch(
        name,
        &PatchParams::default(),
        &Patch::Json::<()>(from_value(patch).unwrap()),
    )
    .await
}

pub async fn add_to_nb(client: Client, name: &str) -> Result<Node, Error> {
    let api: Api<Node> = Api::all(client);
    let mut node = api.get(&name).await.unwrap();
    //let mut annotations = node.metadata.annotations.unwrap_or_default();
    let annotation_key = "node.k8s.linode.com/exclude-from-nb";

    let patch = json!([
        {
            "op": "remove",
            "path": format!("/metadata/annotations/{}", annotation_key.replace("~", "~0").replace("/", "~1"))
        }
    ]);
    println!("Annotations updated for node: {} - Added to NB", name);

    api.patch(
        name,
        &PatchParams::default(),
        &Patch::Json::<()>(from_value(patch).unwrap()),
    )
    .await
}
