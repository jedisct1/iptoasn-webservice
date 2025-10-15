#[cfg(not(target_env = "msvc"))]
use tikv_jemallocator::Jemalloc;

#[cfg(not(target_env = "msvc"))]
#[global_allocator]
static GLOBAL: Jemalloc = Jemalloc;

#[macro_use]
extern crate horrorshow;
#[macro_use]
extern crate log;

mod asns;
mod webservice;

use crate::asns::Asns;
use crate::webservice::WebService;
use clap::{Arg, Command};
use http_body_util::Empty;
use hyper::body::Bytes;
use hyper_rustls::HttpsConnectorBuilder;
use hyper_util::client::legacy::Client;
use hyper_util::rt::TokioExecutor;
use std::sync::{Arc, RwLock};
use std::time::Duration;

#[tokio::main]
async fn main() {
    env_logger::init();

    let matches = Command::new("iptoasn-webservice")
        .version("0.2.5")
        .author("Frank Denis <github@pureftpd.org>")
        .about("IP to ASN webservice")
        .arg(
            Arg::new("listen_addr")
                .short('l')
                .long("listen")
                .value_name("listen_addr")
                .help("Address:port to listen to")
                .default_value("127.0.0.1:53661"),
        )
        .arg(
            Arg::new("db_url")
                .short('u')
                .long("dburl")
                .value_name("db_url")
                .help("URL of the database")
                .default_value("https://iptoasn.com/data/ip2asn-combined.tsv.gz"),
        )
        .arg(
            Arg::new("refresh_delay")
                .short('r')
                .long("refresh")
                .value_name("refresh_delay")
                .help("Database refresh delay (minutes, 0 to disable)")
                .default_value("60"),
        )
        .get_matches();

    let db_url = matches.get_one::<String>("db_url").unwrap();
    let listen_addr = matches.get_one::<String>("listen_addr").unwrap();
    let refresh_delay = matches.get_one::<String>("refresh_delay").unwrap();
    let refresh_delay = match refresh_delay.parse::<u64>() {
        Ok(delay) => delay,
        Err(_) => {
            error!("Invalid refresh delay value: {}", refresh_delay);
            error!("Refresh delay must be a valid number");
            return;
        }
    };

    let http_client = if db_url.starts_with("http://") || db_url.starts_with("https://") {
        let https = HttpsConnectorBuilder::new()
            .with_native_roots()
            .expect("Failed to load native roots")
            .https_or_http()
            .enable_http1()
            .enable_http2()
            .build();
        Some(Client::builder(TokioExecutor::new()).build::<_, Empty<Bytes>>(https))
    } else {
        None
    };

    let asns = match get_asns(db_url, http_client.as_ref()).await {
        Ok(asns) => asns,
        Err(e) => {
            error!("Failed to load initial database: {e}");
            return;
        }
    };
    let asns_arc = Arc::new(RwLock::new(Arc::new(asns)));

    // Only start the refresh task if refresh_delay > 0
    if refresh_delay > 0 {
        let asns_arc_t = asns_arc.clone();
        let db_url_t = db_url.clone();
        let http_client_t = http_client.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(refresh_delay * 60)).await;
                update_asns(&asns_arc_t, &db_url_t, http_client_t.as_ref()).await;
            }
        });
        info!(
            "Automatic database refresh enabled (every {} minutes)",
            refresh_delay
        );
    } else {
        info!("Automatic database refresh disabled");
    }

    WebService::start(asns_arc, listen_addr).await;
}

async fn get_asns(
    db_url: &str,
    http_client: Option<
        &Client<
            hyper_rustls::HttpsConnector<hyper_util::client::legacy::connect::HttpConnector>,
            Empty<Bytes>,
        >,
    >,
) -> Result<Asns, &'static str> {
    info!("Retrieving ASNs");
    let asns = Asns::new(db_url, http_client).await?;
    info!("ASNs loaded");
    Ok(asns)
}

async fn update_asns(
    asns_arc: &Arc<RwLock<Arc<Asns>>>,
    db_url: &str,
    http_client: Option<
        &Client<
            hyper_rustls::HttpsConnector<hyper_util::client::legacy::connect::HttpConnector>,
            Empty<Bytes>,
        >,
    >,
) {
    {
        let empty_asns = Arc::new(Asns {
            asns: std::collections::BTreeSet::new(),
        });
        let mut guard = asns_arc.write().unwrap();
        let old = std::mem::replace(&mut *guard, empty_asns);
        drop(guard);
        drop(old);
    }

    let new_asns = match get_asns(db_url, http_client).await {
        Ok(asns) => Arc::new(asns),
        Err(e) => {
            warn!("Failed to update ASN database: {e}");
            warn!("Continuing with existing data");
            return;
        }
    };

    *asns_arc.write().unwrap() = new_asns;
    info!("ASN database successfully updated");
}
