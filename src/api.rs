use serde_json;
use futures::future::join_all;
use std::sync::Arc;
use tokio::sync::Semaphore;
use tokio::task;
use serde::{Serialize};
use clap::Parser;
use rust_decimal::prelude::*;
use chrono::{NaiveDateTime, DateTime, Utc, Local, TimeZone};
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION};
use reqwest::Client;
use std::env;
use std::sync::LazyLock;
use crate::database::{
    create_maindb_client,
    create_localdb_client,
    localdb_init,
    get_nb_ids,
    get_nbcfg_ids,
    get_nb_by_loc,
    update_db_node,
    update_db_nb,
    update_db_config,
    LocalNodeBalancerListObject,
    NodeBalancerListObject,
    NodeBalancerConfigObject,
    NodeObject
};


static api_version: LazyLock<String> = LazyLock::new(|| {
    env::var("APIVERSION").expect("APIVERSION not set!") 
});
static token: LazyLock<String> = LazyLock::new(|| {
    env::var("TOKEN").expect("TOKEN not set!")
});

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    data: bool,
}

#[derive(serde::Deserialize, Serialize, Debug)]
struct NodeBalancerListData {
    data: Vec<NodeBalancerListObject>,
    page: u64,
    pages: u64,
    results: u64,
}

#[derive(serde::Deserialize, Serialize, Debug)]
struct NodeBalancerConfigData {
    data: Vec<NodeBalancerConfigObject>,
    page: u64,
    pages: u64,
    results: u64,
}

#[derive(serde::Deserialize, Serialize, Debug)]
struct NodeListData {
    data: Vec<NodeObject>,
    page: u64,
    pages: u64,
    results: u64,
}

#[derive(serde::Deserialize, Serialize, Debug)]
struct NodeUpdatePost {
    mode: String,
}

fn epoch_to_dt(e: &String) -> String {
    let timestamp = e.parse::<i64>().unwrap();
    let naive = NaiveDateTime::from_timestamp(timestamp, 0);
    let datetime: DateTime<Utc> = DateTime::from_utc(naive, Utc);
    let newdate = datetime.format("%Y-%m-%d %H:%M:%S");

    newdate.to_string()
}

async pub establish_api_connection() -> Client {
    let auth_header = format!("Bearer {}", token.to_string());
    let mut headers = HeaderMap::new();
    headers.insert(AUTHORIZATION, HeaderValue::from_str(&auth_header).unwrap());
    headers.insert("accept", HeaderValue::from_static("application/json"));

    let client = Client::builder()
        .default_headers(headers)
        .build()?;

    client
}


async pub change_node_mode(api_version, nbid: i32, configid: i32, nodeid: i32) {
    let client = establish_api_connection().await;
    let url = format!("https://api.linode.com/{}/nodebalancers/{}/configs/{}/nodes/{}", api_version.to_string(), nbid, configid, nodeid);
    let config_response = client.post(url)
        .body()
        .send()
        .await?;

}
