use actix_web::http::header::Header;
use actix_web::{
    get, http::header, http::StatusCode, web, App, HttpRequest, HttpResponse, HttpServer, Result,
};
use clap::Arg;
use horrorshow::{html, Template};
use log::{info, warn};
use mime;
use std::net::IpAddr;
use std::str::FromStr;
use std::sync::Arc;
use std::sync::RwLock;
use std::time::Duration;

mod asns;
use asns::ASNs;

async fn get_asns(db_url: &str) -> Result<ASNs, String> {
    info!("Retrieving ASNs");
    let asns = ASNs::new(db_url).await;
    info!("ASNs loaded");
    asns
}

async fn update_asns(asns_arc: &Arc<RwLock<Arc<ASNs>>>, db_url: &str) {
    let asns = match get_asns(db_url).await {
        Ok(asns) => asns,
        Err(e) => {
            warn!("{}", e);
            return;
        }
    };
    *asns_arc.write().unwrap() = Arc::new(asns);
}

enum OutputType {
    Json,
    Html,
}
fn accept_type(req: &HttpRequest) -> OutputType {
    let mut output_type = OutputType::Json;

    if let Ok(header_accept) = header::Accept::parse(req) {
        for mime in header_accept.iter() {
            match (mime.item.type_(), mime.item.subtype()) {
                (mime::TEXT, mime::HTML) => {
                    output_type = OutputType::Html;
                    break;
                }
                (_, mime::JSON) => {
                    output_type = OutputType::Json;
                    break;
                }
                _ => {}
            }
        }
    }
    output_type
}

fn output_json(map: &serde_json::Map<String, serde_json::value::Value>) -> HttpResponse {
    let json = serde_json::to_string(&map).unwrap();
    return HttpResponse::Ok().body(json);
}

fn output_html(map: &serde_json::Map<String, serde_json::value::Value>) -> HttpResponse {
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
    return HttpResponse::Ok().body(html);
}

fn output(
    output_type: &OutputType,
    map: &serde_json::Map<String, serde_json::value::Value>,
) -> HttpResponse {
    match *output_type {
        OutputType::Json => output_json(map),
        _ => output_html(map),
    }
}

#[get("/")]
async fn index() -> HttpResponse {
    HttpResponse::Ok().body("See https://iptoasn.com")
}

#[get("/v1/as/ip/{ip}")]
async fn query(
    asns: web::Data<Arc<RwLock<Arc<ASNs>>>>,
    req: HttpRequest,
    path: web::Path<String>,
) -> HttpResponse {
    let ip_str = path.into_inner();
    let ip = match IpAddr::from_str(ip_str.as_str()) {
        Err(_) => {
            return HttpResponse::build(StatusCode::BAD_REQUEST).body("Invalid IP address");
        }
        Ok(ip) => ip,
    };

    let mut map = serde_json::Map::new();
    map.insert(
        "ip".to_string(),
        serde_json::value::Value::String(ip_str.to_string()),
    );

    let asns = asns.get_ref().read().unwrap();
    let found = match asns.lookup_by_ip(ip) {
        None => {
            map.insert(
                "announced".to_string(),
                serde_json::value::Value::Bool(false),
            );
            return output(&accept_type(&req), &map);
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
    return output(&accept_type(&req), &map);
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init();
    let matches = clap::command!()
        .arg(
            Arg::new("listen_addr")
                .short('l')
                .long("listen")
                .value_name("ip:port")
                .help("Webservice IP and port")
                .num_args(1)
                .default_value("0.0.0.0:53661"),
        )
        .arg(
            Arg::new("db_url")
                .short('u')
                .long("dburl")
                .value_name("url")
                .help("URL of the gzipped database")
                .num_args(1)
                .default_value("https://iptoasn.com/data/ip2asn-combined.tsv.gz"),
        )
        .get_matches();
    let db_url = matches.get_one::<String>("db_url").unwrap().to_owned();
    let listen_addr = matches.get_one::<String>("listen_addr").unwrap();
    let asns = get_asns(&db_url)
        .await
        .expect("Unable to load the initial database");
    let asns_arc = Arc::new(RwLock::new(Arc::new(asns)));
    let asns_arc_copy = asns_arc.clone();
    let asns = web::Data::new(asns_arc);
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(3600)).await;
            update_asns(&asns_arc_copy, &db_url).await;
        }
    });
    HttpServer::new(move || {
        // move counter into the closure
        App::new()
            .app_data(asns.clone()) // <- register the created data
            .service(index)
            .service(query)
    })
    .bind(listen_addr)?
    .run()
    .await
}
