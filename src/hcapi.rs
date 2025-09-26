use serde_json;
use serde::{Serialize};
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION};
use reqwest::Client;
use std::env;
use std::sync::LazyLock;
use std::collections::HashMap;


static api_version: LazyLock<String> = LazyLock::new(|| {
    env::var("APIVERSION").expect("APIVERSION not set!") 
});
static token: LazyLock<String> = LazyLock::new(|| {
    env::var("TOKEN").expect("TOKEN not set!")
});



pub async fn change_node_mode(nbid: &i32, configid: &i32, nodeid: &i32, nodemode: String) -> Result<(), Box<dyn std::error::Error>> {
    println!("####################################CHANGING STATE {}################################", nodemode);
    let auth_header = format!("Bearer {}", token.to_string());
    let mut headers = HeaderMap::new();
    headers.insert(AUTHORIZATION, HeaderValue::from_str(&auth_header).unwrap());
    headers.insert("accept", HeaderValue::from_static("application/json"));

    let mut params = HashMap::new();
    params.insert("mode", nodemode);

    let client = reqwest::Client::builder()
        .default_headers(headers)
        .build()
        .unwrap();

    let url = format!("https://api.linode.com/{}/nodebalancers/{}/configs/{}/nodes/{}", api_version.to_string(), nbid, configid, nodeid);
    let config_response = client
        .put(url)
        .json(&params)
        .send()
        .await?;
    println!("{:#?}", config_response);

    Ok(())
}
