use clap::Parser;
use dashmap::{DashMap, DashSet};
use futures::future::join_all;
use reqwest::Method;
use serde::Deserialize;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tonic::{transport::Server, Request as TonicRequest, Response as TonicResponse, Status};

pub mod mapi {
    pub mod rewrite {
        tonic::include_proto!("mapi.rewrite");
    }
    pub mod classify {
        tonic::include_proto!("mapi.classify");
    }
}

use mapi::classify::classify_plugin_server::{ClassifyPlugin, ClassifyPluginServer};
use mapi::classify::{Issues, Response as ClassifyResponse};
use mapi::rewrite::rewrite_plugin_server::{RewritePlugin, RewritePluginServer};
use mapi::rewrite::{request::Header as RewriteHeader, Request as RewriteRequest};

const FRAGMENT_PREFIX: &str = "mapi-idor-";
const CANARY_LABEL: &str = "__canary__";

#[derive(Parser, Clone)]
pub struct Args {
    #[arg(long, env = "MAPI_IDOR_REWRITE_PORT", default_value_t = 50051)]
    rewrite_port: u16,

    #[arg(long, env = "MAPI_IDOR_CLASSIFY_PORT", default_value_t = 50052)]
    classify_port: u16,

    #[arg(long, env = "MAPI_IDOR_ALT_IDENTITIES")]
    alt_identities: PathBuf,

    #[arg(long, env = "MAPI_IDOR_SIDE_CHANNEL_TIMEOUT_MS", default_value_t = 10_000)]
    side_channel_timeout_ms: u64,

    #[arg(long, env = "MAPI_IDOR_STATE_TTL_SECS", default_value_t = 60)]
    state_ttl_secs: u64,


    #[arg(long, env = "MAPI_IDOR_VERBOSE", default_value_t = false)]
    verbose: bool,

    /// Minimum body-length similarity (min/max of body lengths) required to
    /// flag a side-channel response as IDOR. 1.0 = exact length match required,
    /// 0.0 = ignore body and flag on status alone.
    #[arg(long, env = "MAPI_IDOR_BODY_SIMILARITY_THRESHOLD", default_value_t = 0.8)]
    body_similarity_threshold: f64,

    /// Run the IDOR check on 1 out of every N requests. 0 or 1 means every
    /// request. Higher values reduce load multiplier at the cost of fewer
    /// chances to detect the vuln per run.
    #[arg(long, env = "MAPI_IDOR_SAMPLE_RATE", default_value_t = 10)]
    sample_rate: u64,

    /// The header name to strip on the unauthenticated canary probe. The
    /// canary's purpose is to detect that an endpoint is actually auth-gated
    /// before flagging an IDOR finding.
    #[arg(long, env = "MAPI_IDOR_CANARY_HEADER", default_value = "Authorization")]
    canary_header: String,

    /// Comma-separated list of HTTP methods to IDOR-check. POST is excluded
    /// by default because creates typically return identity-independent
    /// responses ("status: added") and flag false positives. PUT/DELETE are
    /// included because they operate on existing identified resources.
    #[arg(long, env = "MAPI_IDOR_METHODS", default_value = "GET,PUT,DELETE", value_delimiter = ',')]
    methods: Vec<String>,

    /// Disable TLS certificate verification for side-channel requests.
    /// DANGEROUS — only enable when the target uses self-signed or expired
    /// certs in a trusted local environment. With this off, an attacker on
    /// the network path can MITM the connection and harvest the alt
    /// identities (real credentials) the plugin sends.
    #[arg(long, env = "MAPI_IDOR_INSECURE_TLS", default_value_t = false)]
    insecure_tls: bool,
}

#[derive(Debug, Clone, Deserialize)]
struct Identity {
    label: String,
    header: String,
    value: String,
}

#[derive(Debug, Clone)]
struct SideChannelResult {
    label: String,
    status: u16,
    body_len: usize,
}

struct StashEntry {
    method: String,
    path: String,
    canary: SideChannelResult,
    alts: Vec<SideChannelResult>,
    stored_at: Instant,
}

type Stash = Arc<DashMap<String, StashEntry>>;

#[derive(Clone)]
struct PluginState {
    stash: Stash,
    identities: Arc<Vec<Identity>>,
    http: reqwest::Client,
    timeout: Duration,
    verbose: bool,
    body_similarity_threshold: f64,
    sample_rate: u64,
    sample_counter: Arc<AtomicU64>,
    canary_header: Arc<String>,
    blocklist: Arc<DashSet<(String, String)>>,
    allowed_methods: Arc<Vec<String>>,
}

pub struct RewriteSvc {
    state: PluginState,
}

pub struct ClassifySvc {
    state: PluginState,
}

#[tonic::async_trait]
impl RewritePlugin for RewriteSvc {
    async fn rewrite(
        &self,
        request: TonicRequest<RewriteRequest>,
    ) -> Result<TonicResponse<RewriteRequest>, Status> {
        if self.state.verbose {
            eprintln!("received request to rewrite: {:?}", request);
        }
        let mut req = request.into_inner();

        let method = req.http_method.clone();
        let path = path_from_url(&req.url);

        // Method filter: skip non-IDOR-relevant methods (e.g. POST creates).
        if !self
            .state
            .allowed_methods
            .iter()
            .any(|m| m.eq_ignore_ascii_case(&method))
        {
            return Ok(TonicResponse::new(req));
        }

        // Path-shape filter: IDOR is about object-level authorization, so a
        // per-object endpoint should have at least 2 path segments (e.g.
        // /collection/{id}).
        if !has_object_id(&path) {
            return Ok(TonicResponse::new(req));
        }

        let shape = endpoint_shape(&method, &path);

        // Blocklist (publicly-accessible endpoint OR already-flagged shape)
        if self.state.blocklist.contains(&shape) {
            return Ok(TonicResponse::new(req));
        }

        // Sampling: only IDOR-check 1 in N requests. Sampled-out requests pass
        // through untouched (no fragment, no side-channels, no stash).
        let n = self.state.sample_counter.fetch_add(1, Ordering::Relaxed);
        if self.state.sample_rate > 1 && n % self.state.sample_rate != 0 {
            return Ok(TonicResponse::new(req));
        }

        let corr_id = uuid::Uuid::new_v4().simple().to_string();

        let (canary, alts) = fire_side_channels(&self.state, &req).await;

        self.state.stash.insert(
            corr_id.clone(),
            StashEntry {
                method,
                path,
                canary,
                alts,
                stored_at: Instant::now(),
            },
        );

        req.url = format!("{}#{}{}", req.url, FRAGMENT_PREFIX, corr_id);
        Ok(TonicResponse::new(req))
    }
}

#[tonic::async_trait]
impl ClassifyPlugin for ClassifySvc {
    async fn classify(
        &self,
        request: TonicRequest<ClassifyResponse>,
    ) -> Result<TonicResponse<Issues>, Status> {
        if self.state.verbose {
            eprintln!("received request to classify: {:?}", request);
        }
        let resp = request.into_inner();
        let mut issues = Issues::default();

        let Some(corr_id) = extract_corr_id(&resp.request_url) else {
            return Ok(TonicResponse::new(issues));
        };

        let Some((_, entry)) = self.state.stash.remove(&corr_id) else {
            return Ok(TonicResponse::new(issues));
        };

        let original_status = resp.status as u16;
        if !is_success(original_status) {
            return Ok(TonicResponse::new(issues));
        }

        let original_len = resp.body.len();
        let threshold = self.state.body_similarity_threshold;
        let shape = endpoint_shape(&entry.method, &entry.path);

        let canary_sim = body_similarity(original_len, entry.canary.body_len);

        // Canary gate:
        //   * Canary 2xx + body ≈ original → endpoint is publicly accessible.
        //     Blocklist the shape so future requests skip the IDOR machinery.
        //   * Canary non-2xx + body ≈ original → the original's "200" is
        //     effectively an error response in disguise (similar shape to the
        //     no-auth denial). Suppress this finding but DON'T blocklist —
        //     a valid ID on the same shape may return real data later.
        if canary_sim >= threshold {
            if is_success(entry.canary.status) {
                self.state.blocklist.insert(shape.clone());
                eprintln!(
                    "blocklisting {} {} from future IDOR checks: endpoint appears public (canary HTTP {}, ~{:.0}% body match)",
                    shape.0, shape.1, entry.canary.status, canary_sim * 100.0
                );
            } else {
                eprintln!(
                    "skipping {} {}: original response shape ≈ no-auth canary (~{:.0}% body match) — likely an error/not-found masquerading as 2xx",
                    entry.method, entry.path, canary_sim * 100.0
                );
            }
            return Ok(TonicResponse::new(issues));
        }

        // Endpoint is auth-gated and original is data-bearing. Check each alt.
        // Emit a canonical, shape-based summary so mAPI's own dedup collapses
        // all hits across fuzzed concrete IDs into a single finding per shape.
        // Concrete URL + body-match % are surfaced to stderr only.
        for r in entry.alts {
            if r.status != original_status || !is_success(r.status) {
                continue;
            }
            let sim = body_similarity(original_len, r.body_len);
            if sim < threshold {
                continue;
            }
            issues.issues.push(mapi::classify::issues::Issue {
                summary: format!(
                    "IDOR/BOLA: '{}' returned HTTP {} on {} {}",
                    r.label, r.status, shape.0, shape.1
                ),
            });
            eprintln!(
                "emitted IDOR for '{}' on {} {} (concrete {} {}, ~{:.0}% body match)",
                r.label, shape.0, shape.1, entry.method, entry.path, sim * 100.0
            );
        }

        Ok(TonicResponse::new(issues))
    }
}

async fn fire_side_channels(
    state: &PluginState,
    req: &RewriteRequest,
) -> (SideChannelResult, Vec<SideChannelResult>) {
    let Ok(method) = Method::from_bytes(req.http_method.as_bytes()) else {
        let empty = SideChannelResult {
            label: CANARY_LABEL.to_string(),
            status: 0,
            body_len: 0,
        };
        return (empty, vec![]);
    };

    let base_url = strip_fragment(&req.url);

    // Canary: strip the configured auth header, no replacement.
    let canary_fut = fire_one(
        state,
        method.clone(),
        base_url.clone(),
        req.body.clone(),
        req.headers.clone(),
        CANARY_LABEL.to_string(),
        state.canary_header.as_str().to_string(),
        None,
    );

    // Alt identities in parallel.
    let alt_futs = state.identities.iter().map(|identity| {
        fire_one(
            state,
            method.clone(),
            base_url.clone(),
            req.body.clone(),
            req.headers.clone(),
            identity.label.clone(),
            identity.header.clone(),
            Some(identity.value.clone()),
        )
    });

    let (canary, alts) = tokio::join!(canary_fut, join_all(alt_futs));
    (canary, alts)
}

async fn fire_one(
    state: &PluginState,
    method: Method,
    url: String,
    body: Vec<u8>,
    original_headers: Vec<RewriteHeader>,
    label: String,
    swap_header: String,
    swap_value: Option<String>,
) -> SideChannelResult {
    let mut builder = state.http.request(method, &url).timeout(state.timeout);

    for h in &original_headers {
        let name = String::from_utf8_lossy(&h.name);
        if name.eq_ignore_ascii_case(&swap_header) {
            continue;
        }
        builder = builder.header(name.as_ref(), h.value.as_slice());
    }

    if let Some(v) = swap_value.as_ref() {
        if !v.is_empty() {
            builder = builder.header(swap_header.as_str(), v.as_str());
        }
    }

    if !body.is_empty() {
        builder = builder.body(body);
    }

    let (status, body_len) = match builder.send().await {
        Ok(r) => {
            let st = r.status().as_u16();
            let len = r.bytes().await.map(|b| b.len()).unwrap_or(0);
            (st, len)
        }
        Err(e) => {
            eprintln!("side-channel error for '{}': {}", label, e);
            (0, 0)
        }
    };

    SideChannelResult {
        label,
        status,
        body_len,
    }
}

fn extract_corr_id(url: &str) -> Option<String> {
    let frag = url.split_once('#')?.1;
    frag.strip_prefix(FRAGMENT_PREFIX).map(|s| s.to_string())
}

fn strip_fragment(url: &str) -> String {
    match url.split_once('#') {
        Some((base, _)) => base.to_string(),
        None => url.to_string(),
    }
}

fn path_from_url(url: &str) -> String {
    let no_frag = strip_fragment(url);
    let no_query = match no_frag.split_once('?') {
        Some((p, _)) => p.to_string(),
        None => no_frag,
    };
    if let Some(rest) = no_query.find("://").map(|i| &no_query[i + 3..]) {
        if let Some(slash) = rest.find('/') {
            return rest[slash..].to_string();
        }
    }
    no_query
}

fn is_success(status: u16) -> bool {
    (200..300).contains(&status)
}

fn body_similarity(a: usize, b: usize) -> f64 {
    if a == 0 && b == 0 {
        return 1.0;
    }
    let (min, max) = if a < b { (a, b) } else { (b, a) };
    min as f64 / max as f64
}

fn has_object_id(path: &str) -> bool {
    path.split('/').filter(|s| !s.is_empty()).count() >= 2
}

/// Canonicalize a request to its endpoint "shape" by uppercasing the method
/// and replacing the last path segment with `*`. Used as the blocklist key
/// so a single finding (or canary verdict) deduplicates all siblings.
fn endpoint_shape(method: &str, path: &str) -> (String, String) {
    let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    let shape_path = if segments.len() <= 1 {
        path.to_string()
    } else {
        format!("/{}/*", segments[..segments.len() - 1].join("/"))
    };
    (method.to_ascii_uppercase(), shape_path)
}

fn spawn_evictor(stash: Stash, ttl: Duration) {
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_secs(30));
        loop {
            tick.tick().await;
            let now = Instant::now();
            stash.retain(|_, entry| now.duration_since(entry.stored_at) < ttl);
        }
    });
}

fn load_identities(path: &PathBuf) -> Result<Vec<Identity>, Box<dyn std::error::Error>> {
    let bytes = std::fs::read(path)?;
    let identities: Vec<Identity> = serde_json::from_slice(&bytes)?;
    if identities.is_empty() {
        return Err("identities file is empty".into());
    }
    Ok(identities)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    let identities = load_identities(&args.alt_identities)?;
    eprintln!(
        "loaded {} alternate identities; sample rate 1-in-{}; canary strips '{}'; methods {:?}",
        identities.len(),
        args.sample_rate.max(1),
        args.canary_header,
        args.methods,
    );

    if args.insecure_tls {
        eprintln!(
            "WARNING: TLS verification disabled for side-channel requests (MAPI_IDOR_INSECURE_TLS=true). \
             Alt identities (real credentials) may be exposed to MITM. Only use against trusted local targets."
        );
    }

    let state = PluginState {
        stash: Arc::new(DashMap::new()),
        identities: Arc::new(identities),
        http: reqwest::Client::builder()
            .danger_accept_invalid_certs(args.insecure_tls)
            .build()?,
        timeout: Duration::from_millis(args.side_channel_timeout_ms),
        verbose: args.verbose,
        body_similarity_threshold: args.body_similarity_threshold,
        sample_rate: args.sample_rate,
        sample_counter: Arc::new(AtomicU64::new(0)),
        canary_header: Arc::new(args.canary_header),
        blocklist: Arc::new(DashSet::new()),
        allowed_methods: Arc::new(args.methods),
    };

    spawn_evictor(state.stash.clone(), Duration::from_secs(args.state_ttl_secs));

    let rewrite_addr: std::net::SocketAddr =
        format!("0.0.0.0:{}", args.rewrite_port).parse().unwrap();
    let classify_addr: std::net::SocketAddr =
        format!("0.0.0.0:{}", args.classify_port).parse().unwrap();

    eprintln!(
        "Listening: rewrite on {}, classify on {}",
        rewrite_addr, classify_addr
    );

    let rewrite_state = state.clone();
    let rewrite_server = Server::builder()
        .add_service(RewritePluginServer::new(RewriteSvc {
            state: rewrite_state,
        }))
        .serve(rewrite_addr);

    let classify_server = Server::builder()
        .add_service(ClassifyPluginServer::new(ClassifySvc { state }))
        .serve(classify_addr);

    tokio::try_join!(rewrite_server, classify_server)?;
    Ok(())
}
