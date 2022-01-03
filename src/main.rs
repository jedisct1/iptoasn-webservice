#[macro_use]
extern crate horrorshow;
#[macro_use]
extern crate log;
#[macro_use]
extern crate router;
#[macro_use]
extern crate clap;

mod asns;
mod webservice;

use crate::asns::*;
use crate::webservice::*;
use clap::Arg;
use std::sync::{Arc, RwLock};
use std::thread;
use std::time::Duration;

fn get_asns(db_url: &str) -> Result<ASNs, &'static str> {
    info!("Retrieving ASNs");
    let asns = ASNs::new(db_url);
    info!("ASNs loaded");
    asns
}

fn update_asns(asns_arc: &Arc<RwLock<Arc<ASNs>>>, db_url: &str) {
    let asns = match get_asns(db_url) {
        Ok(asns) => asns,
        Err(e) => {
            warn!("{}", e);
            return;
        }
    };
    *asns_arc.write().unwrap() = Arc::new(asns);
}

fn main() {
    let matches = app_from_crate!()
        .arg(
            Arg::new("listen_addr")
                .short('l')
                .long("listen")
                .value_name("ip:port")
                .help("Webservice IP and port")
                .takes_value(true)
                .default_value("0.0.0.0:53661"),
        )
        .arg(
            Arg::new("db_url")
                .short('u')
                .long("dburl")
                .value_name("url")
                .help("URL of the gzipped database")
                .takes_value(true)
                .default_value("https://iptoasn.com/data/ip2asn-combined.tsv.gz"),
        )
        .get_matches();
    let db_url = matches.value_of("db_url").unwrap().to_owned();
    let listen_addr = matches.value_of("listen_addr").unwrap();
    let asns = get_asns(&db_url).expect("Unable to load the initial database");
    let asns_arc = Arc::new(RwLock::new(Arc::new(asns)));
    let asns_arc_copy = asns_arc.clone();
    thread::spawn(move || loop {
        thread::sleep(Duration::from_secs(3600));
        update_asns(&asns_arc_copy, &db_url);
    });
    info!("Starting the webservice");
    WebService::start(asns_arc, listen_addr);
}
