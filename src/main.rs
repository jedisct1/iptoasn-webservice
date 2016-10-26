#![cfg_attr(feature="clippy", feature(plugin))]
#![cfg_attr(feature="clippy", plugin(clippy))]
#![feature(btree_range, collections_bound)]

extern crate clap;
extern crate flate2;
extern crate iron;
#[macro_use]
extern crate router;
extern crate hyper;
#[macro_use]
extern crate log;
extern crate serde;
extern crate serde_json;
#[macro_use(slog_error, slog_info, slog_trace, slog_log, o)]
extern crate slog;

mod asns;
mod webservice;

use asns::*;
use clap::{Arg, App};
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

fn main() {
    logger_init();
    let matches = App::new("iptoasn webservice")
        .version("0.1")
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
    let db_url = matches.value_of("db_url").unwrap();
    let listen_addr = matches.value_of("listen_addr").unwrap();
    let asns = match ASNs::new(db_url) {
        Ok(asns) => asns,
        Err(err) => panic!(format!("{} [{}]", err, db_url)),
    };
    info!("Starting the webservice");
    WebService::start(asns, listen_addr);
}
