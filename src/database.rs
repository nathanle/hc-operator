use tokio_postgres::{Row, Client, Error};
use std::collections::HashMap;
use openssl::ssl::{SslConnector, SslMethod, SslVerifyMode};
use postgres_openssl::MakeTlsConnector;
use std::env;
use serde::{Serialize};
use std::sync::LazyLock;


static maindb_pw: LazyLock<String> = std::sync::LazyLock::new(|| { env::var("MAINDB_PASSWORD").expect("MAINDB_PASSWORD not set!") });
static localdb_pw: LazyLock<String> = std::sync::LazyLock::new(|| { env::var("LOCALDB_PASSWORD").expect("LOCALDB_PASSWORD not set!") });
static maindb_hostport: LazyLock<String> = std::sync::LazyLock::new(|| { env::var("MAINDB_HOSTPORT").expect("MAINDB_HOSTPORT not set!") });
static localdb_hostport: LazyLock<String> = std::sync::LazyLock::new(|| { env::var("LOCALDB_HOSTPORT").expect("LOCALDB_HOSTPORT not set!") });

#[derive(Debug)]
pub struct Nodebalancer {
    _id: i32,
    ip_address: String,
    port: i32,
}

#[derive(serde::Deserialize, Serialize, Debug)]
pub struct NodeBalancerListObject {
    client_conn_throttle: i32,
    created: String,
    hostname: String,
    pub id: i32,
    ipv4: String,
    ipv6: String,
    label: String,
    lke_cluster: Option<LkeCluster>,
    region: String,
    r#type: String,
    updated: String,
}

#[derive(serde::Deserialize, Serialize, Debug)]
pub struct LocalNodeBalancerListObject {
    pub nb_id: i32,
    pub ipv4: String,
    pub region: String,
    pub lke_id: i32,
}

#[derive(serde::Deserialize, Serialize, Debug)]
pub struct NodeObject {
    address: String,
    config_id: i32,
    id: i32,
    label: String,
    mode: String,
    nodebalancer_id: i32,
    status: String,
    weight: i32 
}

#[derive(serde::Deserialize, Serialize, Debug)]
pub struct LkeCluster{
    id: i32,
    label: String,
    r#type: String,
    url: String,
}

impl Default for LkeCluster {
    fn default() -> Self {
        LkeCluster {
            id: 0,
            label: String::new(),
            r#type: String::new(),
            url: String::new(),
        }
    }
}

#[derive(serde::Deserialize, Serialize, Debug)]
pub struct NodeBalancerConfigObject {
    algorithm: String,
    check: String,
    check_attempts: i32,
    check_body: String,
    check_interval: i32,
    check_passive: bool,
    check_path: String,
    check_timeout: i32,
    cipher_suite: String,
    pub id: i32,
    pub nodebalancer_id: i32,
    nodes_status: NodeStatus,
    port: i32,
    protocol: String,
    proxy_protocol: String,
    stickiness: String,
    udp_check_port: i32,
    udp_session_timeout: i32, 
}

#[derive(serde::Deserialize, Serialize, Debug)]
pub struct NodeStatus {
    down: i32,
    up: i32,
}

async fn create_connector() -> MakeTlsConnector {
    let mut builder = SslConnector::builder(SslMethod::tls()).expect("unable to create sslconnector builder");
    builder.set_ca_file("/tmp/ca.cert").expect("unable to load ca.cert");
    builder.set_verify(SslVerifyMode::NONE);
    let connector = MakeTlsConnector::new(builder.build());

    connector
} 

pub async fn create_localdb_client() -> Client {
    let connector = create_connector().await;

    let url = format!("postgresql://akmadmin:{}@{}/defaultdb", localdb_pw.to_string(), localdb_hostport.to_string());
    let Ok((client, connection)) = tokio_postgres::connect(&url, connector).await else { panic!("Client failure.") };
    tokio::spawn(async move {
        if let Err(e) = connection.await {
            eprintln!("connection error: {}", e);
        }
    });

    client 

}

pub async fn create_maindb_client() -> Client {
    let connector = create_connector().await;

    let url = format!("postgresql://akmadmin:{}@{}/defaultdb", maindb_pw.to_string(), maindb_hostport.to_string());
    let Ok((client, connection)) = tokio_postgres::connect(&url, connector).await else { panic!("Client failure.") };
    tokio::spawn(async move {
        if let Err(e) = connection.await {
            eprintln!("connection error: {}", e);
        }
    });

    client 

}
pub async fn localdb_init() -> Result<(), Box<dyn std::error::Error>> {
    let mut connection = create_localdb_client().await;
    let main_table = connection.batch_execute("
        CREATE TABLE IF NOT EXISTS nodebalancer (
            nb_id INTEGER NOT NULL,
            ipv4 VARCHAR NOT NULL,
            region VARCHAR NOT NULL,
            lke_id INTEGER,
            PRIMARY KEY (nb_id)
            );
    ");

    match main_table.await {
        Ok(success) => println!("Nodebalancer table availabe"),
        Err(e) => println!("{:?}", e),
        }

    let nb_cfg_conn  = connection.batch_execute("
        CREATE TABLE IF NOT EXISTS nodebalancer_config  (
            id INTEGER NOT NULL,
            algorithm VARCHAR NOT NULL,
            port INTEGER NOT NULL,
            up INTEGER NOT NULL,
            down INTEGER NOT NULL,
            nodebalancer_id INTEGER NOT NULL REFERENCES nodebalancer,
            PRIMARY KEY (id, nodebalancer_id)
            );
    ");
    match nb_cfg_conn.await {
        Ok(success) => println!("Nodebalancer config table availabe"),
        Err(e) => println!("{:?}", e),
        }


    let node_table  = connection.batch_execute("
        CREATE TABLE IF NOT EXISTS node  (
            id INTEGER NOT NULL,
            address VARCHAR NOT NULL,
            status VARCHAR NOT NULL,
            config_id INTEGER NOT NULL,
            nodebalancer_id INTEGER NOT NULL REFERENCES nodebalancer,
            PRIMARY KEY (id, nodebalancer_id)
            );
    ");
    match node_table.await {
        Ok(success) => println!("Node table available"),
        Err(e) => println!("{:?}", e),
        }

    Ok(())

}

pub async fn update_db_nb(nodebalancers: LocalNodeBalancerListObject) -> Result<(), Box<dyn std::error::Error>> {
    let mut connection = create_localdb_client().await;
    //println!("{:#?}", nodebalancers);

    let update = connection.execute(
            "INSERT INTO nodebalancer (nb_id, ipv4, region, lke_id) VALUES ($1, $2, $3, $4)",
            &[&nodebalancers.nb_id, &nodebalancers.ipv4, &nodebalancers.region, &nodebalancers.lke_id],
    ).await;

    match update {
        Ok(success) => (),
        Err(e) => {
            if e.to_string().contains("duplicate key value violates unique constraint") {
                ();
            } else {
                println!("{:?}", e);
            }
        }
    }

    Ok(())

}

pub async fn get_nb_ids() -> Result<Vec<Row>, Error> {
    let mut node_connection = create_localdb_client().await;
    let nb_table = node_connection.query(
        "SELECT nb_id FROM nodebalancer", &[],
    ).await;

    Ok(nb_table?)

}

pub async fn get_nb_by_loc(loc: String) -> Result<Vec<Row>, Error> {
    let mut node_connection = create_maindb_client().await;
    let nb_table = node_connection.query(
        "SELECT * FROM nodebalancer where region = $1", &[&loc],
    ).await;

    Ok(nb_table?)

}

pub async fn get_nbcfg_ids() -> Result<Vec<Row>, Error> {
    let mut node_connection = create_localdb_client().await;
    let nb_table = node_connection.query(
        "SELECT id, nodebalancer_id FROM nodebalancer_config", &[],
    ).await;

    Ok(nb_table?)

}

pub async fn get_by_node_ip_nbcfg(ip: &String, port: &i32) -> Result<Vec<Row>, Error> {
    //let mut new_ip: &String = ip;
    let searchpattern = format!("%{}%", &ip);
    println!("IP SENT TO DB {}", &searchpattern);
    let mut node_connection = create_localdb_client().await;
    let nb_table = node_connection.query(
        "select * from node INNER JOIN nodebalancer_config ON node.config_id = nodebalancer_config.id where address LIKE $1 AND port = $2;", &[&searchpattern, &port],
    ).await;

    //println!("{:#?}", nb_table);

    Ok(nb_table?)

}

pub async fn get_by_node_ip_nb(ip: String) -> Result<Vec<Row>, Error> {
    //FIX the same as function above
    let mut node_connection = create_localdb_client().await;
    let nb_table = node_connection.query(
        "select * from node INNER JOIN nodebalancer ON node.nodebalancer_id = nodebalancer.nb_id where address like '%$1%';", &[&ip],
    ).await;

    Ok(nb_table?)

}

pub async fn update_db_node(node: NodeObject) -> Result<(), Box<dyn std::error::Error>> {
    let mut node_connection = create_localdb_client().await;
    let nb_table = node_connection.execute(
            "INSERT INTO node (id, address, status, config_id, nodebalancer_id) VALUES ($1, $2, $3, $4, $5)",
            &[&node.id, &node.address, &node.status, &node.config_id, &node.nodebalancer_id],
    ).await;

    Ok(())
}

pub async fn update_db_config(nodebalancer_config: NodeBalancerConfigObject) -> Result<(), Box<dyn std::error::Error>> {
    let mut config_connection = create_localdb_client().await;
    //println!("{:#?}", nodebalancer_config);

    let nb_cfg_table = config_connection.execute(
            "INSERT INTO nodebalancer_config (id, algorithm, port, up, down, nodebalancer_id) VALUES ($1, $2, $3, $4, $5, $6)",
            &[&nodebalancer_config.id, &nodebalancer_config.algorithm, &nodebalancer_config.port, &nodebalancer_config.nodes_status.up, &nodebalancer_config.nodes_status.down, &nodebalancer_config.nodebalancer_id],
    ).await;

    match nb_cfg_table {
        //Ok(success) => println!("Nodebalancer config row updated"),
        Ok(success) => (),
        Err(e) => {
            if e.to_string().contains("duplicate key value violates unique constraint") {
                ();
                //println!("Config already in DB: {} - Node: {}", &nodebalancer_config.id, &nodebalancer_config.nodebalancer_id);
            } else {
                println!("{:?}", e);
            }
        }
    }

    Ok(())


}
