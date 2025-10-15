use flate2::read::GzDecoder;
use http::Request;
use http_body_util::{BodyExt, Empty};
use hyper::body::Bytes;
use hyper::{Method, StatusCode};
use hyper_rustls::HttpsConnectorBuilder;
use hyper_util::client::legacy::Client;
use hyper_util::rt::TokioExecutor;
use std::cmp::{Eq, Ord, Ordering, PartialEq, PartialOrd};
use std::collections::BTreeSet;
use std::io::{prelude::*, BufReader};
use std::net::IpAddr;
use std::ops::Bound::{Included, Unbounded};
use std::str::FromStr;

#[derive(Debug)]
pub struct Asn {
    pub first_ip: IpAddr,
    pub last_ip: IpAddr,
    pub number: u32,
    pub country: String,
    pub description: String,
}

impl PartialEq for Asn {
    fn eq(&self, other: &Self) -> bool {
        self.first_ip == other.first_ip
    }
}

impl Eq for Asn {}

impl Ord for Asn {
    fn cmp(&self, other: &Self) -> Ordering {
        self.first_ip.cmp(&other.first_ip)
    }
}

impl PartialOrd for Asn {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Asn {
    const fn from_single_ip(ip: IpAddr) -> Self {
        Self {
            first_ip: ip,
            last_ip: ip,
            number: 0,
            country: String::new(),
            description: String::new(),
        }
    }
}

pub struct Asns {
    pub asns: BTreeSet<Asn>, // Make it public
}

impl Asns {
    pub async fn new(
        url: &str,
        http_client: Option<
            &Client<
                hyper_rustls::HttpsConnector<hyper_util::client::legacy::connect::HttpConnector>,
                Empty<Bytes>,
            >,
        >,
    ) -> Result<Self, &'static str> {
        let bytes = if url.starts_with("file://") {
            // Handle local file URL
            let path = url.trim_start_matches("file://");
            info!("Loading the database from file://{}", path);
            match std::fs::read(path) {
                Ok(content) => Bytes::from(content),
                Err(e) => {
                    error!("Unable to read the database: {}", e);
                    return Err("Unable to read the database");
                }
            }
        } else if url.starts_with("http://") || url.starts_with("https://") {
            // Handle HTTP or HTTPS URL
            info!("Loading the database from {}", url);

            let client = match http_client {
                Some(c) => c,
                None => {
                    let https = HttpsConnectorBuilder::new()
                        .with_native_roots()
                        .expect("Failed to load native roots")
                        .https_or_http()
                        .enable_http1()
                        .enable_http2()
                        .build();

                    // This creates a temporary client - not ideal but maintains backward compatibility
                    &Client::builder(TokioExecutor::new()).build::<_, Empty<Bytes>>(https)
                }
            };

            // Create the request
            let req = Request::builder()
                .method(Method::GET)
                .uri(url)
                .header("User-Agent", "iptoasn-webservice/0.2.5")
                .body(Empty::<Bytes>::new())
                .map_err(|e| {
                    error!("Failed to create request: {}", e);
                    "Failed to create request"
                })?;

            // Send the request and get the response
            match client.request(req).await {
                Ok(res) => {
                    if res.status() != StatusCode::OK {
                        error!("Unable to load the database, status: {}", res.status());
                        warn!("HTTP request failed, attempting to use cached data");

                        // Try fallback sources on HTTP errors too
                        let fallback_paths = [
                            "cache/ip2asn-combined.tsv.gz",
                            "ip2asn-combined.tsv.gz",
                            "test_data.tsv.gz",
                        ];

                        for path in &fallback_paths {
                            match std::fs::read(path) {
                                Ok(content) => {
                                    info!("Successfully loaded fallback data from: {}", path);
                                    return Self::parse_data(Bytes::from(content));
                                }
                                Err(_) => {
                                    debug!("Fallback file not found: {}", path);
                                }
                            }
                        }

                        return Err("Unable to load the database and no fallback data available");
                    }

                    // Collect the response body
                    let body = res.into_body();
                    match BodyExt::collect(body).await {
                        Ok(collected) => collected.to_bytes(),
                        Err(e) => {
                            error!("Unable to read response body: {}", e);
                            return Err("Unable to read response body");
                        }
                    }
                }
                Err(e) => {
                    error!("Failed to send request: {}", e);
                    warn!("Network request failed, attempting to use cached data");

                    // Try multiple fallback sources in order of preference
                    let fallback_paths = [
                        "cache/ip2asn-combined.tsv.gz", // Primary cache location
                        "ip2asn-combined.tsv.gz",       // Local copy
                        "test_data.tsv.gz",             // Test data fallback
                    ];

                    for path in &fallback_paths {
                        match std::fs::read(path) {
                            Ok(content) => {
                                info!("Successfully loaded fallback data from: {}", path);
                                return Self::parse_data(Bytes::from(content));
                            }
                            Err(_) => {
                                debug!("Fallback file not found: {}", path);
                            }
                        }
                    }

                    error!("No fallback data sources available");
                    return Err("Failed to load database from URL and all fallback sources");
                }
            }
        } else {
            error!("Unsupported URL scheme: {}", url);
            return Err("Unsupported URL scheme");
        };

        // Save successful download to cache
        if url.starts_with("http://") || url.starts_with("https://") {
            Self::save_to_cache(&bytes);
        }

        Self::parse_data(bytes)
    }

    fn save_to_cache(bytes: &Bytes) {
        // Create cache directory if it doesn't exist
        if let Err(e) = std::fs::create_dir_all("cache") {
            warn!("Failed to create cache directory: {}", e);
            return;
        }

        // Save the downloaded data to cache
        match std::fs::write("cache/ip2asn-combined.tsv.gz", bytes) {
            Ok(()) => info!("Successfully cached database to cache/ip2asn-combined.tsv.gz"),
            Err(e) => warn!("Failed to cache database: {}", e),
        }
    }

    fn parse_data(bytes: Bytes) -> Result<Self, &'static str> {
        // Use BufReader to stream line-by-line instead of reading entire file into String
        let decoder = GzDecoder::new(bytes.as_ref());
        let reader = BufReader::new(decoder);

        let mut asns = BTreeSet::new();
        let mut line_count = 0;
        let mut error_count = 0;

        for line_result in reader.lines() {
            let line = match line_result {
                Ok(l) => l,
                Err(e) => {
                    error!("Error reading line: {}", e);
                    return Err("Unable to decompress the database");
                }
            };

            if line.trim().is_empty() {
                continue;
            }

            line_count += 1;

            let mut parts = line.split('\t');
            let first_ip = match parts.next().and_then(|s| IpAddr::from_str(s).ok()) {
                Some(ip) => ip,
                None => {
                    error_count += 1;
                    if error_count <= 10 {
                        warn!("Invalid first IP address in line {}: {}", line_count, line);
                    }
                    continue;
                }
            };
            let last_ip = match parts.next().and_then(|s| IpAddr::from_str(s).ok()) {
                Some(ip) => ip,
                None => {
                    error_count += 1;
                    if error_count <= 10 {
                        warn!("Invalid last IP address in line {}: {}", line_count, line);
                    }
                    continue;
                }
            };
            let number = match parts.next().and_then(|s| u32::from_str(s).ok()) {
                Some(num) => num,
                None => {
                    error_count += 1;
                    if error_count <= 10 {
                        warn!("Invalid ASN number in line {}: {}", line_count, line);
                    }
                    continue;
                }
            };
            let country = parts.next().unwrap_or("").to_owned();
            let description = parts.next().unwrap_or("").to_owned();

            let asn = Asn {
                first_ip,
                last_ip,
                number,
                country,
                description,
            };
            asns.insert(asn);
        }

        if error_count > 10 {
            warn!(
                "Total of {} malformed lines encountered (showing first 10)",
                error_count
            );
        }

        info!(
            "Database loaded with {} entries from {} lines",
            asns.len(),
            line_count
        );
        Ok(Self { asns })
    }

    pub fn lookup_by_ip(&self, ip: IpAddr) -> Option<&Asn> {
        let fasn = Asn::from_single_ip(ip);
        match self.asns.range((Unbounded, Included(&fasn))).next_back() {
            Some(found) if ip <= found.last_ip && found.number > 0 => Some(found),
            _ => None,
        }
    }
}
