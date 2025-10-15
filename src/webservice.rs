use crate::asns::Asns;
use horrorshow::prelude::*;
use http::header::{ACCEPT, CACHE_CONTROL, CONTENT_TYPE, EXPIRES, VARY};
use http::{HeaderMap, HeaderValue, Method, Request, Response, StatusCode};
use http_body_util::Full;
use hyper::body::Bytes;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper_util::rt::TokioIo;
use serde::{Deserialize, Serialize};
use std::convert::Infallible;
use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::{Arc, RwLock};
use time::macros::format_description;
use time::OffsetDateTime;
use tokio::net::TcpListener;

const TTL: u32 = 86_400;

enum OutputType {
    Json,
    Html,
}

#[derive(Serialize, Deserialize)]
struct IpLookupResponse {
    ip: String,
    announced: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    first_ip: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    last_ip: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    as_number: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    as_country_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    as_description: Option<String>,
}

pub struct WebService;

impl WebService {
    async fn handle_request(
        req: Request<hyper::body::Incoming>,
        asns_arc: Arc<RwLock<Arc<Asns>>>,
        remote_addr: SocketAddr,
    ) -> Result<Response<Full<Bytes>>, Infallible> {
        let method = req.method();
        let uri = req.uri().path();

        match (method, uri) {
            (&Method::GET, "/") => Ok(Self::index()),
            (&Method::GET, "/v1/as/ip") => {
                let client_ip = Self::extract_client_ip(req.headers(), remote_addr);
                Self::ip_lookup(&client_ip, req.headers(), asns_arc)
            }
            (&Method::GET, path) if path.starts_with("/v1/as/ip/") => {
                let ip_s = path.strip_prefix("/v1/as/ip/").unwrap_or("");
                Self::ip_lookup(ip_s, req.headers(), asns_arc)
            }
            _ => {
                let mut response = Response::new(Full::new(Bytes::from("Not Found")));
                *response.status_mut() = StatusCode::NOT_FOUND;
                Ok(response)
            }
        }
    }

    fn index() -> Response<Full<Bytes>> {
        let mut response = Response::new(Full::new(Bytes::from("iptoasn-webservice\n")));
        response.headers_mut().insert(
            CONTENT_TYPE,
            HeaderValue::from_static("text/plain; charset=utf-8"),
        );
        *response.status_mut() = StatusCode::OK;
        response
    }

    fn extract_client_ip(headers: &HeaderMap, remote_addr: SocketAddr) -> String {
        if let Some(real_ip) = headers.get("x-real-ip") {
            if let Ok(ip_str) = real_ip.to_str() {
                return ip_str.to_string();
            }
        }

        if let Some(forwarded_for) = headers.get("x-forwarded-for") {
            if let Ok(ip_str) = forwarded_for.to_str() {
                let first_ip = ip_str.split(',').next().unwrap_or("").trim();
                if !first_ip.is_empty() {
                    return first_ip.to_string();
                }
            }
        }

        remote_addr.ip().to_string()
    }

    fn accept_type(headers: &HeaderMap) -> OutputType {
        if let Some(accept) = headers.get(ACCEPT) {
            if let Ok(accept_str) = accept.to_str() {
                if accept_str.contains("application/json") {
                    return OutputType::Json;
                }
            }
        }
        OutputType::Html
    }

    fn cache_headers(headers: &mut HeaderMap) {
        let now = OffsetDateTime::now_utc();
        let expires = now + time::Duration::seconds(TTL as i64);

        let format = format_description!(
            "[weekday repr:short], [day] [month repr:short] [year] [hour]:[minute]:[second] GMT"
        );
        let expires_str = expires.format(&format).unwrap();

        headers.insert(
            CACHE_CONTROL,
            HeaderValue::from_str(&format!("max-age={}", TTL)).unwrap(),
        );
        headers.insert(EXPIRES, HeaderValue::from_str(&expires_str).unwrap());
        headers.insert(VARY, HeaderValue::from_static("Accept"));
    }

    fn output_json(response: &IpLookupResponse) -> Response<Full<Bytes>> {
        let json = serde_json::to_string(&response).unwrap();
        let mut response = Response::new(Full::new(Bytes::from(json)));

        response.headers_mut().insert(
            CONTENT_TYPE,
            HeaderValue::from_static("application/json; charset=utf-8"),
        );
        Self::cache_headers(response.headers_mut());
        *response.status_mut() = StatusCode::OK;

        response
    }

    fn output_html(response: &IpLookupResponse) -> Response<Full<Bytes>> {
        let html = html! {
            head {
                title : "iptoasn lookup";
                meta(name="viewport", content="width=device-width, initial-scale=1");
                link(rel="stylesheet", href="https://maxcdn.bootstrapcdn.com/bootstrap/4.0.0-alpha.5/css/bootstrap.min.css", integrity="sha384-AysaV+vQoT3kOAXZkl02PThvDr8HYKPZhNT5h/CXfBThSRXQ6jW5DO2ekP5ViFdi", crossorigin="anonymous");
                style : "body { margin: 1em 4em }";
            }
            body(class="container-fluid") {
                header {
                    h1 : format_args!("Information for IP address: {}", response.ip);
                }
                table {
                    tr {
                        th : "Announced";
                        td {
                            @ if response.announced {
                                : "Yes";
                            } else {
                                : "No";
                            }
                        }
                    }
                    @ if response.announced {
                        tr {
                            th : "AS Number";
                            td : format_args!("AS{}", response.as_number.unwrap());
                        }
                        tr {
                            th : "AS Range";
                            td : format_args!("{} - {}", response.first_ip.as_ref().unwrap(), response.last_ip.as_ref().unwrap());
                        }
                        tr {
                            th : "AS Country Code";
                            td : response.as_country_code.as_ref().unwrap();
                        }
                        tr {
                            th : "AS Description";
                            td : response.as_description.as_ref().unwrap();
                        }
                    }
                }
                footer {
                    p { small {
                        : "Powered by ";
                        a(href="https://iptoasn.com") : "iptoasn.com";
                    } }
                }
            }
        }.into_string()
            .unwrap();
        let html = format!("<!DOCTYPE html>\n<html>{html}</html>");

        let mut response = Response::new(Full::new(Bytes::from(html)));
        response.headers_mut().insert(
            CONTENT_TYPE,
            HeaderValue::from_static("text/html; charset=utf-8"),
        );
        Self::cache_headers(response.headers_mut());
        *response.status_mut() = StatusCode::OK;

        response
    }

    fn output(output_type: &OutputType, response: &IpLookupResponse) -> Response<Full<Bytes>> {
        match *output_type {
            OutputType::Json => Self::output_json(response),
            OutputType::Html => Self::output_html(response),
        }
    }

    fn ip_lookup(
        ip_s: &str,
        headers: &HeaderMap,
        asns_arc: Arc<RwLock<Arc<Asns>>>,
    ) -> Result<Response<Full<Bytes>>, Infallible> {
        let ip = match std::net::IpAddr::from_str(ip_s) {
            Err(_) => {
                let response = IpLookupResponse {
                    ip: ip_s.to_owned(),
                    announced: false,
                    first_ip: None,
                    last_ip: None,
                    as_number: None,
                    as_country_code: None,
                    as_description: None,
                };
                return Ok(Self::output(&Self::accept_type(headers), &response));
            }
            Ok(ip) => ip,
        };

        let response = {
            let asns_guard = asns_arc.read().unwrap();

            let found = asns_guard.lookup_by_ip(ip);

            match found {
                None => IpLookupResponse {
                    ip: ip.to_string(),
                    announced: false,
                    first_ip: None,
                    last_ip: None,
                    as_number: None,
                    as_country_code: None,
                    as_description: None,
                },
                Some(found) => IpLookupResponse {
                    ip: ip.to_string(),
                    announced: true,
                    first_ip: Some(found.first_ip.to_string()),
                    last_ip: Some(found.last_ip.to_string()),
                    as_number: Some(found.number),
                    as_country_code: Some(found.country.clone()),
                    as_description: Some(found.description.clone()),
                },
            }
            // asns_guard (and read lock) dropped here
        };

        Ok(Self::output(&Self::accept_type(headers), &response))
    }

    pub async fn start(asns_arc: Arc<RwLock<Arc<Asns>>>, listen_addr: &str) {
        let addr: SocketAddr = listen_addr.parse().expect("Could not parse socket address");
        let listener = match TcpListener::bind(addr).await {
            Ok(listener) => listener,
            Err(e) => {
                log::error!("Failed to bind to {}: {}", addr, e);
                return;
            }
        };

        log::info!("webservice ready");

        loop {
            let (tcp, remote_addr) = match listener.accept().await {
                Ok(conn) => conn,
                Err(e) => {
                    log::error!("Failed to accept connection: {}", e);
                    continue;
                }
            };
            let io = TokioIo::new(tcp);
            let asns_arc = asns_arc.clone();

            tokio::task::spawn(async move {
                let service = service_fn(move |req| {
                    let asns_arc = asns_arc.clone();
                    async move { Self::handle_request(req, asns_arc, remote_addr).await }
                });

                if let Err(err) = http1::Builder::new().serve_connection(io, service).await {
                    log::error!("Error serving connection: {:?}", err);
                }
            });
        }
    }
}
