use async_mutex::Mutex;
use clap::error::{Error, ErrorKind};
use clap::Parser;
use reqwest::header::{HeaderName, HeaderValue};
use reqwest::{header::CONTENT_TYPE, Client, StatusCode};
use std::str::FromStr;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tonic::{transport::Server, Response, Status};
use url::Url;
#[macro_use]
extern crate serde_derive;

/// Requests an access token from an OpenID server and then injects the access
/// token as an header into the API being tested by mapi.
/// See https://mayhem4api.forallsecure.com/docs/rewrite.html for more details
/// about rewrite plugins
#[derive(Parser, Clone)]
pub struct Args {
    /// The GRPC port that will be listened on for rewrite requests from the `mapi` program.
    /// Specify 0 to have an open port auto-selected.
    /// May be specified using environment variable MAPI_PLUGIN_PORT.
    #[arg(long, default_value_t = 50051, env)]
    mapi_plugin_port: u16,

    /// The URL of the openid-connect `/token` endpoint from which access tokens
    /// will be fetched
    #[arg(long, env)]
    oauth_token_url: Url,

    /// This option provides the body of the POST request sent to the `/token`
    /// endpoint in order to fetch an access token. The data will be urlencoded.
    /// Multiple calls will be joined with the `&` symbol and the full body of
    /// the HTTP request will be sent with a `content-type` of
    /// `application/x-www-form-urlencoded` This switch is a minimalist copy of
    /// curl's `switch of the same name.
    #[arg(long = "data-urlencode", env, value_parser(try_parse_data_urlencode))]
    oauth_data_urlencodes: Vec<String>,

    /// A header to be sent to the oauth enpoint
    #[arg(
        long = "header",
        env = "MAPI_OAUTH_REQUEST_HEADER",
        value_parser(try_parse_header)
    )]
    oauth_request_headers: Vec<(HeaderName, HeaderValue)>,

    /// The name of the HTTP header which will have the access token attached
    /// when requests are sent to the API under test
    #[arg(
        long,
        default_value = "authorization",
        env,
        value_parser(try_parse_header_name)
    )]
    api_under_test_header_name: HeaderName,

    /// A prefix to prepended to the access token and set as the value of the
    /// header field from `api_under_test_header_name`
    #[arg(long, env)]
    api_under_test_header_prefix: Option<HeaderValue>,
}

pub mod mapi {
    pub mod rewrite {
        tonic::include_proto!("mapi.rewrite");
    }
}

use mapi::rewrite::rewrite_plugin_server::{RewritePlugin, RewritePluginServer};
pub struct MyRewriterPlugin {
    args: Args,
    access_token: Arc<Mutex<Option<String>>>,
    refresh_time: Arc<Mutex<SystemTime>>,
}

// https://openid.net/specs/openid-connect-core-1_0.html#rfc.section.12.2
#[derive(Deserialize)]
struct AccessTokenResponse {
    access_token: String,
    expires_in: Option<u64>,
}

impl MyRewriterPlugin {
    async fn ensure_fresh_access_token(&self) {
        let mut refresh_time = self.refresh_time.lock().await;

        if SystemTime::now() < *refresh_time {
            return;
        }

        println!("Refreshing Access Token");

        let client = Client::new();

        let mut request = client
            .post(self.args.oauth_token_url.clone())
            .header(CONTENT_TYPE, "application/x-www-form-urlencoded");
        for (header_name, header_value) in self.args.oauth_request_headers.iter() {
            request = request.header(header_name, header_value);
        }

        let mut body = vec![];
        for oauth_data_urlencode in self.args.oauth_data_urlencodes.iter() {
            body.push(oauth_data_urlencode.clone());
        }

        let response = request
            .body(body.join("&"))
            .send()
            .await
            .unwrap_or_else(|e| panic!("Unable to fetch access token - {}", e));

        let status = response.status();
        let body = response.text().await;

        if StatusCode::OK != status {
            panic!(
                "Access token request failed with status {} and response '{:?}'",
                status, body
            );
        }
        let body = body.unwrap_or_else(|e| panic!("Unable to access token response - {}", e));
        let access_token_response = serde_json::from_str::<AccessTokenResponse>(&body)
            .unwrap_or_else(|e| {
                panic!("Unable to deserialize access token response - {}", e);
            });

        let expires_in = if let Some(expires_in) = access_token_response.expires_in {
            Duration::from_secs(expires_in / 2)
        } else {
            Duration::from_secs(30)
        };
        *refresh_time = SystemTime::now() + expires_in;

        *self.access_token.lock().await = Some(access_token_response.access_token);
    }
}

#[tonic::async_trait]
impl RewritePlugin for MyRewriterPlugin {
    async fn rewrite(
        &self,
        request: tonic::Request<mapi::rewrite::Request>,
    ) -> Result<Response<mapi::rewrite::Request>, Status> {
        self.ensure_fresh_access_token().await;

        let mut req = request.into_inner();

        if let Some(access_token) = self.access_token.lock().await.as_ref() {
            let mut header_value = access_token.as_bytes().to_vec();
            if let Some(prefix) = &self.args.api_under_test_header_prefix {
                header_value = [prefix.as_bytes().to_vec(), header_value].concat();
            }
            req.headers.push(mapi::rewrite::request::Header {
                name: self
                    .args
                    .api_under_test_header_name
                    .as_str()
                    .as_bytes()
                    .to_vec(),
                value: header_value,
            });
        }

        Ok(Response::new(req))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    let port = if args.mapi_plugin_port == 0 {
        portpicker::pick_unused_port().expect("Unable to find a free port.")
    } else {
        args.mapi_plugin_port
    };

    // MUST print the port as the first line for `mapi` to know how to connect
    // to this plugin
    println!("{}", port);

    let rewriter = MyRewriterPlugin {
        args,
        access_token: Arc::new(Mutex::new(None)),
        refresh_time: Arc::new(Mutex::new(UNIX_EPOCH.clone())),
    };

    rewriter.ensure_fresh_access_token().await;

    let addr = format!("127.0.0.1:{}", port).parse().unwrap();
    println!("Listening at {}", addr);

    Server::builder()
        .add_service(RewritePluginServer::new(rewriter))
        .serve(addr)
        .await?;

    Ok(())
}

fn try_parse_header(header: &str) -> Result<(HeaderName, HeaderValue), Error> {
    let split: Vec<&str> = header.splitn(2, ":").collect();
    if split.len() != 2 {
        return Err(Error::raw(
            ErrorKind::InvalidValue,
            format!(
                "A header was provided that does not conform to the \
            pattern <header_name>: <header_value> - {}",
                header
            ),
        ));
    }
    let name = try_parse_header_name(split[0])?;
    let value = HeaderValue::from_str(split[1].trim_start()).map_err(|e| {
        Error::raw(
            ErrorKind::InvalidValue,
            format!(
                "Unable to parse header value '{}' as a valid header value - {}",
                split[1].trim_start(),
                e
            ),
        )
    })?;

    Ok((name, value))
}

fn try_parse_header_name(header_name: &str) -> Result<HeaderName, Error> {
    Ok(HeaderName::from_str(header_name).map_err(|e| {
        Error::raw(
            ErrorKind::InvalidValue,
            format!(
                "Unable to parse header name '{}' as a valid header name - {}",
                header_name, e
            ),
        )
    })?)
}

fn try_parse_data_urlencode(data: &str) -> Result<String, Error> {
    let split: Vec<&str> = data.splitn(2, '=').collect();

    let mut prefix = "".to_string();

    let chunk_to_encode = if split.len() > 1 {
        if !split[0].is_empty() {
            prefix = split[0].to_string() + "=";
        }
        split[1]
    } else {
        split[0]
    };

    let mut encoded = "".to_string();
    for chunk in form_urlencoded::byte_serialize(chunk_to_encode.as_bytes()) {
        encoded += chunk;
    }
    Ok(prefix + &encoded)
}
