mod cli;
mod state;
mod config;

use crate::cli::*;
use crate::state::RemoteCluster;
use log::{warn, debug};
use state::State;
use std::collections::HashMap;
use std::error::Error;
use std::sync::Arc;
use structopt::StructOpt;
use warp::Filter;
use crate::config::ShellConfig;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    pretty_env_logger::init();

    let opt = CliOptions::from_args();
    debug!("Effective {:?}", opt);

    let config = ShellConfig::new();
    warn!("Config {:?}", config);

    let mut clusters = HashMap::new();

    let active = if config.clusters().is_empty() {
        let cluster = RemoteCluster::new(opt.connection_string, opt.username, opt.password);
        clusters.insert("default".into(), cluster);
        String::from("default")
    } else {
        let mut first = None;
        for (k, v) in config.clusters() {
            let cluster = RemoteCluster::new(v.connstr().into(), v.username().into(), v.password().into());
            clusters.insert(k.clone(), cluster);
            if first.is_none() {
                first = Some(k.clone());
            }
        }
        first.unwrap()
    };

    let state = Arc::new(State::new(clusters, active));

    if opt.ui {
        tokio::task::spawn(async {
            let hello =
                warp::path!("hello" / String).map(|name| format!("Couchbase says, {}!", name));
            warp::serve(hello).run(([127, 0, 0, 1], 1908)).await;
        });
    }

    let mut syncer = nu::EnvironmentSyncer::new();
    let mut context = nu::create_default_context(&mut syncer)?;
    context.add_commands(vec![
        // Performs analytics queries
        nu::whole_stream_command(Analytics::new(state.clone())),
        // Performs kv get operations
        nu::whole_stream_command(Get::new(state.clone())),
        // Displays cluster manager node infos
        nu::whole_stream_command(Nodes::new(state.clone())),
        // Displays cluster manager bucket infos
        nu::whole_stream_command(Buckets::new(state.clone())),
        // Performs n1ql queries
        nu::whole_stream_command(Query::new(state.clone())),
        // Manages local cluster references
        nu::whole_stream_command(Clusters::new(state.clone())),
    ]);

    nu::cli(Some(syncer), Some(context)).await
}

#[derive(Debug, StructOpt)]
#[structopt(
    name = "The Couchbase Shell",
    about = "Alternative Shell and UI for Couchbase Server and Cloud"
)]
struct CliOptions {
    #[structopt(
        short = "c",
        long = "connstring",
        default_value = "couchbase://localhost"
    )]
    connection_string: String,
    #[structopt(long = "ui")]
    ui: bool,
    #[structopt(short = "u", long = "username", default_value = "Administrator")]
    username: String,
    #[structopt(short = "p", long = "password", default_value = "password")]
    password: String,
}
