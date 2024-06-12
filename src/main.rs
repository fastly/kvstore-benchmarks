use fastly::http::{header, Method, StatusCode};
use fastly::secret_store;
use fastly::ConfigStore;
use fastly::KVStore;
use fastly::{mime, Error, Request, Response};
use rand::distributions::{Alphanumeric, DistString};
use serde::Deserialize;
use std::time::Instant;

#[derive(Deserialize)]
struct CodeParams {
    region: Option<String>,
    action: Option<String>,
    operation: Option<String>,
}

#[fastly::main]
fn main(req: Request) -> Result<Response, Error> {
    // Log service version
    println!(
        "FASTLY_SERVICE_VERSION: {}",
        std::env::var("FASTLY_SERVICE_VERSION").unwrap_or_else(|_| String::new())
    );

    // Filter request methods...
    match req.get_method() {
        // Block requests with unexpected methods
        &Method::POST | &Method::PUT | &Method::PATCH | &Method::DELETE => {
            return Ok(Response::from_status(StatusCode::METHOD_NOT_ALLOWED)
                .with_header(header::ALLOW, "GET, HEAD, PURGE")
                .with_body_text_plain("This method is not allowed\n"))
        }

        // Let any other requests through
        _ => (),
    };

    // Pattern match on the path...
    const RUNS: i32 = 101;
    match req.get_path() {
        // If request is to the `/` path, serve the HTML
        "/" => {
            let params: CodeParams = req.get_query().unwrap();
            let fastly_pop =
                std::env::var("FASTLY_POP").unwrap_or_else(|_| "localhost".to_string());
            let html = include_str!("index.html").replace("FASTLY_POP", &fastly_pop);

            Ok(Response::from_status(StatusCode::OK)
                .with_content_type(mime::TEXT_HTML_UTF_8)
                .with_body(html))
        }
        "/config-store/csv" => {
            let params: CodeParams = req.get_query().unwrap();
            let string = Alphanumeric.sample_string(&mut rand::thread_rng(), 32);

            let mut config_latency: [f64; RUNS as usize] = [0.0; RUNS as usize];

            let config_store = ConfigStore::open("myconfigstore");

            for n in 1..RUNS {
                let key = match params.action.clone().unwrap().as_ref() {
                    "hit" => "DEFAULT_KEY_".to_string() + &n.to_string(),
                    _ => string.clone() + &n.to_string(),
                };

                let now = Instant::now();
                let _entry = config_store.get(&key);
                let elapsed_time = now.elapsed();
                config_latency[n as usize] = elapsed_time.as_secs_f64() * 1000.0;
            }
            let mut csv = "latency\n".to_owned();
            for n in 1..RUNS {
                csv = csv + &config_latency[n as usize].to_string() + "\n";
            }

            Ok(Response::from_body(csv)
                .with_status(StatusCode::OK)
                .with_header("Access-Control-Allow-Origin", "*"))
        }
        "/secret-store/csv" => {
            let params: CodeParams = req.get_query().unwrap();
            let string = Alphanumeric.sample_string(&mut rand::thread_rng(), 32);

            let mut secret_latency: [f64; RUNS as usize] = [0.0; RUNS as usize];

            let secret_store =
                secret_store::SecretStore::open("compute-store-read-write-latencies")?;

            for n in 1..RUNS {
                let elapsed_time = match params.action.clone().unwrap().as_ref() {
                    "hit" => {
                        let key = "DEFAULT_KEY_".to_string() + &n.to_string();
                        let now = Instant::now();
                        let _entry = secret_store.get(&key).unwrap().plaintext();
                        now.elapsed()
                    }
                    _ => {
                        let key = string.clone() + &n.to_string();
                        let now = Instant::now();
                        let _entry = secret_store.get(&key);
                        now.elapsed()
                    }
                };
                secret_latency[n as usize] = elapsed_time.as_secs_f64() * 1000.0;
            }
            let mut csv = "latency\n".to_owned();
            for n in 1..RUNS {
                csv = csv + &secret_latency[n as usize].to_string() + "\n";
            }

            Ok(Response::from_body(csv)
                .with_status(StatusCode::OK)
                .with_header("Access-Control-Allow-Origin", "*"))
        }
        "/kv-store/csv" => {
            let params: CodeParams = req.get_query().unwrap();
            let string = Alphanumeric.sample_string(&mut rand::thread_rng(), 32);

            let mut kv_latency: [f64; RUNS as usize] = [0.0; RUNS as usize];

            let region = match params.region.unwrap().as_ref() {
                "US" => "-US",
                "EU" => "-EU",
                "ASIA" => "-ASIA",
                "AUS" => "-AUS",
                _ => "",
            };
            let mut kv_store = KVStore::open(&format!(
                "compute-store-read-write-latencies-prod{}",
                region
            ))?
            .unwrap();

            for n in 1..RUNS {
                if params.operation.clone().unwrap() == "write" && n >= 40 {
                    break;
                }

                let key = match params.action.clone().unwrap().as_ref() {
                    "hit" => "DEFAULT_KEY_".to_string() + &n.to_string(),
                    _ => string.clone() + &n.to_string(),
                };

                let now = Instant::now();
                if params.operation.clone().unwrap() == "write" {
                    kv_store.insert(&key, "SOME_VALUE");
                } else {
                    let _entry = kv_store.lookup_str(&key)?;
                };
                let elapsed_time = now.elapsed();
                kv_latency[n as usize] = elapsed_time.as_secs_f64() * 1000.0;
            }
            let mut csv = "latency\n".to_owned();
            for n in 1..RUNS {
                if params.operation.clone().unwrap() == "write" && n >= 41 {
                    break;
                }
                csv = csv + &kv_latency[n as usize].to_string() + "\n";
            }

            Ok(Response::from_body(csv)
                .with_status(StatusCode::OK)
                .with_header("Access-Control-Allow-Origin", "*"))
        }
        // Catch all other requests and return a 404.
        _ => Ok(Response::from_status(StatusCode::NOT_FOUND)
            .with_body_text_plain("The page you requested could not be found\n")),
    }
}
