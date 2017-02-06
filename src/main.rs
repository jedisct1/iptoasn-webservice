#![cfg_attr(feature="clippy", feature(plugin))]
#![cfg_attr(feature="clippy", plugin(clippy))]
#![feature(btree_range, collections_bound)]

extern crate clap;
extern crate flate2;
extern crate iron;
#[macro_use]
extern crate router;
#[macro_use]
extern crate horrorshow;
extern crate hyper;
#[macro_use]
extern crate log;
extern crate serde;
extern crate serde_json;
#[macro_use(o)]
extern crate slog;

mod asns;
mod webservice;

use asns::*;
use clap::{Arg, App};
use std::sync::{Arc, RwLock};
use std::thread;
use std::time::Duration;
use webservice::*;

fn logger_init() {
    extern crate slog_term;
    extern crate slog_stdlog;
    extern crate slog_envlogger;

    use slog::DrainExt;

    let drain = slog_envlogger::new(slog_term::streamer().build());
    let root_logger = slog::Logger::root(drain.fuse(),
                                         o!("place" => move |info: &slog::Record| {
                                             format!("{}:{} {}",
                                             info.file(), info.line(), info.module()
                                         )}));
    slog_stdlog::set_logger(root_logger.clone()).unwrap();
}

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
    logger_init();
    let matches = App::new("iptoasn webservice")
        .version("0.2.0")
        .author("Frank Denis")
        .about("Webservice for https://iptoasn.com")
        .arg(Arg::with_name("listen_addr")
            .short("l")
            .long("listen")
            .value_name("ip:port")
            .help("Webservice IP and port")
            .takes_value(true)
            .default_value("0.0.0.0:53661"))
        .arg(Arg::with_name("db_url")
            .short("u")
            .long("dburl")
            .value_name("url")
            .help("URL of the gzipped database")
            .takes_value(true)
            .default_value("https://iptoasn.com/data/ip2asn-combined.tsv.gz"))
        .get_matches();
    let db_url = matches.value_of("db_url").unwrap().to_owned();
    let listen_addr = matches.value_of("listen_addr").unwrap();
    let asns = get_asns(&db_url).expect("Unable to load the initial database");
    let asns_arc = Arc::new(RwLock::new(Arc::new(asns)));
    let asns_arc_copy = asns_arc.clone();
    thread::spawn(move || {
        loop {
            thread::sleep(Duration::from_secs(3600));
            update_asns(&asns_arc_copy, &db_url);
        }
    });
    info!("Starting the webservice");
    WebService::start(asns_arc, listen_addr);
}
