use crate::asns::*;
use horrorshow::prelude::*;
use iron::headers::{Accept, CacheControl, CacheDirective, Expires, HttpDate, Vary};
use iron::mime::*;
use iron::modifiers::Header;
use iron::prelude::*;
use iron::status;
use iron::{typemap, BeforeMiddleware};
use router::Router;

use std::net::IpAddr;
use std::str::FromStr;
use std::sync::{Arc, RwLock};
use time::{self, Duration};
use unicase::UniCase;

const TTL: u32 = 86_400;

struct ASNsMiddleware {
    asns_arc: Arc<RwLock<Arc<ASNs>>>,
}

impl typemap::Key for ASNsMiddleware {
    type Value = Arc<ASNs>;
}

impl ASNsMiddleware {
    fn new(asns_arc: Arc<RwLock<Arc<ASNs>>>) -> ASNsMiddleware {
        ASNsMiddleware { asns_arc }
    }
}

impl BeforeMiddleware for ASNsMiddleware {
    fn before(&self, req: &mut Request<'_, '_>) -> IronResult<()> {
        req.extensions
            .insert::<ASNsMiddleware>(self.asns_arc.read().unwrap().clone());
        Ok(())
    }
}

enum OutputType {
    Json,
    Html,
}

pub struct WebService;

impl WebService {
    fn index(_: &mut Request<'_, '_>) -> IronResult<Response> {
        Ok(Response::with((
            status::Ok,
            Mime(
                TopLevel::Text,
                SubLevel::Plain,
                vec![(Attr::Charset, Value::Utf8)],
            ),
            Header(CacheControl(vec![
                CacheDirective::Public,
                CacheDirective::MaxAge(TTL),
            ])),
            Header(Expires(HttpDate(
                time::now() + Duration::seconds(TTL.into()),
            ))),
            "See https://iptoasn.com",
        )))
    }

    fn accept_type(req: &Request<'_, '_>) -> OutputType {
        let mut output_type = OutputType::Json;
        if let Some(header_accept) = req.headers.get::<Accept>() {
            for header in header_accept.iter() {
                match header.item {
                    Mime(TopLevel::Text, SubLevel::Html, _) => {
                        output_type = OutputType::Html;
                        break;
                    }
                    Mime(_, SubLevel::Json, _) => {
                        output_type = OutputType::Json;
                        break;
                    }
                    _ => {}
                }
            }
        }
        output_type
    }

    fn output_json(
        map: &serde_json::Map<String, serde_json::value::Value>,
        cache_headers: (Header<CacheControl>, Header<Expires>),
        vary_header: Header<Vary>,
    ) -> IronResult<Response> {
        let json = serde_json::to_string(&map).unwrap();
        let mime_json = Mime(
            TopLevel::Application,
            SubLevel::Json,
            vec![(Attr::Charset, Value::Utf8)],
        );
        Ok(Response::with((
            status::Ok,
            mime_json,
            cache_headers.0,
            cache_headers.1,
            vary_header,
            json,
        )))
    }

    fn output_html(
        map: &serde_json::Map<String, serde_json::value::Value>,
        cache_headers: (Header<CacheControl>, Header<Expires>),
        vary_header: Header<Vary>,
    ) -> IronResult<Response> {
        let mime_html = Mime(
            TopLevel::Text,
            SubLevel::Html,
            vec![(Attr::Charset, Value::Utf8)],
        );
        let html = html!{
            head {
                title { : "iptoasn lookup" }
                meta(name="viewport", content="width=device-widthinitial-scale=1");
                link(rel="stylesheet", href="https://maxcdn.bootstrapcdn.com/bootstrap/4.0.0-alpha.5/css/bootstrap.min.css", integrity="sha384-AysaV+vQoT3kOAXZkl02PThvDr8HYKPZhNT5h/CXfBThSRXQ6jW5DO2ekP5ViFdi", crossorigin="anonymous");
                style {
                    : "body { margin: 1em 4em }"
                }
            }
            body(class="container-fluid") {
                header {
                    h1 { : format_args!("Information for IP address: {}", map.get("ip").unwrap().as_str().unwrap()) }
                }
                table {
                    tr {
                        th { : "Announced" }
                        td { : format_args!("{}", if map.get("announced")
                            .unwrap().as_bool().unwrap() { "Yes" } else { "No" }) }
                    }
                    @ if map.get("announced").unwrap().as_bool().unwrap() {
                        tr {
                            th { : "First IP" }
                            td { : format_args!("{}", map.get("first_ip")
                                .unwrap().as_str().unwrap()) }
                        }
                        tr {
                            th { : "Last IP" }
                            td { : format_args!("{}", map.get("last_ip")
                                .unwrap().as_str().unwrap()) }
                        }
                        tr {
                            th { : "AS Number" }
                            td { : format_args!("{}", map.get("as_number")
                                .unwrap().as_u64().unwrap()) }
                        }
                        tr {
                            th { : "AS Country code" }
                            td { : format_args!("{}", map.get("as_country_code")
                                .unwrap().as_str().unwrap()) }
                        }
                        tr {
                            th { : "AS Description" }
                            td { : format_args!("{}", map.get("as_description")
                                .unwrap().as_str().unwrap()) }
                        }
                    }
                }
            }
        }.into_string()
            .unwrap();
        let html = format!("<!DOCTYPE html>\n<html>{}</html>", html);
        Ok(Response::with((
            status::Ok,
            mime_html,
            cache_headers.0,
            cache_headers.1,
            vary_header,
            html,
        )))
    }

    fn output(
        output_type: &OutputType,
        map: &serde_json::Map<String, serde_json::value::Value>,
        cache_headers: (Header<CacheControl>, Header<Expires>),
        vary_header: Header<Vary>,
    ) -> IronResult<Response> {
        match *output_type {
            OutputType::Json => Self::output_json(map, cache_headers, vary_header),
            _ => Self::output_html(map, cache_headers, vary_header),
        }
    }

    fn ip_lookup(req: &mut Request<'_, '_>) -> IronResult<Response> {
        let mime_text = Mime(
            TopLevel::Text,
            SubLevel::Plain,
            vec![(Attr::Charset, Value::Utf8)],
        );
        let cache_headers = (
            Header(CacheControl(vec![
                CacheDirective::Public,
                CacheDirective::MaxAge(TTL),
            ])),
            Header(Expires(HttpDate(
                time::now() + Duration::seconds(TTL.into()),
            ))),
        );
        let vary_header = Header(Vary::Items(vec![
            UniCase::from_str("accept-encoding").unwrap(),
            UniCase::from_str("accept").unwrap(),
        ]));
        let ip_str = match req.extensions.get::<Router>().unwrap().find("ip") {
            None => {
                let response = Response::with((
                    status::BadRequest,
                    mime_text,
                    cache_headers,
                    "Missing IP address",
                ));
                return Ok(response);
            }
            Some(ip_str) => ip_str,
        };
        let ip = match IpAddr::from_str(ip_str) {
            Err(_) => {
                return Ok(Response::with((
                    status::BadRequest,
                    mime_text,
                    cache_headers,
                    "Invalid IP address",
                )));
            }
            Ok(ip) => ip,
        };
        let asns = req.extensions.get::<ASNsMiddleware>().unwrap();
        let mut map = serde_json::Map::new();
        map.insert(
            "ip".to_string(),
            serde_json::value::Value::String(ip_str.to_string()),
        );
        let found = match asns.lookup_by_ip(ip) {
            None => {
                map.insert(
                    "announced".to_string(),
                    serde_json::value::Value::Bool(false),
                );
                return Self::output(&Self::accept_type(req), &map, cache_headers, vary_header);
            }
            Some(found) => found,
        };
        map.insert(
            "announced".to_string(),
            serde_json::value::Value::Bool(true),
        );
        map.insert(
            "first_ip".to_string(),
            serde_json::value::Value::String(found.first_ip.to_string()),
        );
        map.insert(
            "last_ip".to_string(),
            serde_json::value::Value::String(found.last_ip.to_string()),
        );
        map.insert(
            "as_number".to_string(),
            serde_json::value::Value::Number(serde_json::Number::from(found.number)),
        );
        map.insert(
            "as_country_code".to_string(),
            serde_json::value::Value::String(found.country.clone()),
        );
        map.insert(
            "as_description".to_string(),
            serde_json::value::Value::String(found.description.clone()),
        );
        Self::output(&Self::accept_type(req), &map, cache_headers, vary_header)
    }

    pub fn start(asns_arc: Arc<RwLock<Arc<ASNs>>>, listen_addr: &str) {
        let router = router!(index: get "/" => Self::index,
                             ip_lookup: get "/v1/as/ip/:ip" => Self::ip_lookup);
        let mut chain = Chain::new(router);
        let asns_middleware = ASNsMiddleware::new(asns_arc);
        chain.link_before(asns_middleware);
        warn!("webservice ready");
        Iron::new(chain).http(listen_addr).unwrap();
    }
}
