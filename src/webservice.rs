use asns::*;
use iron::{BeforeMiddleware, typemap};
use iron::headers::{CacheControl, CacheDirective};
use iron::mime::*;
use iron::modifiers::Header;
use iron::prelude::*;
use iron::status;
use router::Router;
use serde_json;
use std::net::IpAddr;
use std::str::FromStr;
use std::sync::Arc;

const TTL: u32 = 86400;

struct ASNsMiddleware {
    asns: Arc<ASNs>,
}

impl typemap::Key for ASNsMiddleware {
    type Value = Arc<ASNs>;
}

impl ASNsMiddleware {
    fn new(asns: ASNs) -> ASNsMiddleware {
        ASNsMiddleware { asns: Arc::new(asns) }
    }
}

impl BeforeMiddleware for ASNsMiddleware {
    fn before(&self, req: &mut Request) -> IronResult<()> {
        req.extensions.insert::<ASNsMiddleware>(self.asns.clone());
        Ok(())
    }
}

pub struct WebService;

impl WebService {
    fn index(_: &mut Request) -> IronResult<Response> {
        Ok(Response::with((status::Ok,
                           Mime(TopLevel::Text,
                                SubLevel::Plain,
                                vec![(Attr::Charset, Value::Utf8)]),
                           Header(CacheControl(vec![CacheDirective::Public,
                                                    CacheDirective::MaxAge(TTL)])),
                           "See https://iptoasn.com")))
    }

    fn ip_lookup(req: &mut Request) -> IronResult<Response> {
        let mime_text = Mime(TopLevel::Text,
                             SubLevel::Plain,
                             vec![(Attr::Charset, Value::Utf8)]);
        let mime_json = Mime(TopLevel::Application,
                             SubLevel::Json,
                             vec![(Attr::Charset, Value::Utf8)]);
        let cache_header = Header(CacheControl(vec![CacheDirective::Public,
                                                    CacheDirective::MaxAge(TTL)]));
        let ip_str = match req.extensions.get::<Router>().unwrap().find("ip") {
            None => {
                let response = Response::with((status::BadRequest,
                                               mime_text,
                                               cache_header,
                                               "Missing IP address"));
                return Ok(response);
            }
            Some(ip_str) => ip_str,
        };
        let ip = match IpAddr::from_str(ip_str) {
            Err(_) => {
                return Ok(Response::with((status::BadRequest,
                                          mime_text,
                                          cache_header,
                                          "Invalid IP address")));
            }
            Ok(ip) => ip,
        };
        let asns = req.extensions.get::<ASNsMiddleware>().unwrap();
        let found = match asns.lookup_by_ip(ip) {
            None => {
                let mut map = serde_json::Map::new();
                map.insert("announced", serde_json::value::Value::Bool(false));
                let json = serde_json::to_string(&map).unwrap();
                return Ok(Response::with((status::Ok, mime_json, cache_header, json)));
            }
            Some(found) => found,
        };
        let mut map = serde_json::Map::new();
        map.insert("announced", serde_json::value::Value::Bool(true));
        map.insert("first_ip",
                   serde_json::value::Value::String(found.first_ip.to_string()));
        map.insert("last_ip",
                   serde_json::value::Value::String(found.last_ip.to_string()));
        map.insert("as_number",
                   serde_json::value::Value::U64(found.number as u64));
        map.insert("as_country_code",
                   serde_json::value::Value::String(found.country.clone()));
        map.insert("as_description",
                   serde_json::value::Value::String(found.description.clone()));
        let json = serde_json::to_string(&map).unwrap();
        Ok(Response::with((status::Ok, mime_json, cache_header, json)))
    }

    pub fn start(asns: ASNs, listen_addr: &str) {
        let router = router!(index: get "/" => Self::index,
                             ip_lookup: get "/v1/as/ip/:ip" => Self::ip_lookup);
        let mut chain = Chain::new(router);
        let asns_middleware = ASNsMiddleware::new(asns);
        chain.link_before(asns_middleware);
        warn!("webservice ready");
        Iron::new(chain).http(listen_addr).unwrap();
    }
}
