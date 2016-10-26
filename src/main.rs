#![cfg_attr(feature="clippy", feature(plugin))]
#![cfg_attr(feature="clippy", plugin(clippy))]
#![feature(btree_range, collections_bound)]

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
    info!("Starting");
    let asns = ASNs::new("https://iptoasn.com/data/ip2asn-v4.tsv.gz");
    WebService::start(asns, "0.0.0.0:53661");
}
