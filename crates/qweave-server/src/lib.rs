//! Read-only HTTP server for an interactive factor-evaluation report. Serves the
//! table JSON the Vue frontend consumes plus the frontend itself (embedded into
//! the binary when built). Data is held in memory (see [`ReportData`]).

mod assets;
mod data;

use std::net::{Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::sync::Arc;

use axum::Json;
use axum::Router;
use axum::extract::{Path, State};
use axum::http::{StatusCode, Uri, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use serde_json::{Value, json};
use tower_http::cors::CorsLayer;
use tower_http::services::{ServeDir, ServeFile};

pub use data::{DataError, ReportData};

#[derive(Clone)]
struct AppState {
    data: Arc<ReportData>,
    shutdown: Arc<tokio::sync::Notify>,
}

/// Build the report router. The frontend is served from `assets_override` (a
/// `dist` dir, for dev) when given, else from the embedded build, else an
/// API-only placeholder.
pub fn router(data: ReportData, assets_override: Option<PathBuf>) -> Router {
    router_with_shutdown(data, assets_override, Arc::new(tokio::sync::Notify::new()))
}

fn router_with_shutdown(
    data: ReportData,
    assets_override: Option<PathBuf>,
    shutdown_signal: Arc<tokio::sync::Notify>,
) -> Router {
    let api = Router::new()
        .route("/api/meta", get(meta))
        .route("/api/summary", get(summary))
        .route("/api/factor/{name}", get(factor))
        .route("/api/shutdown", post(shutdown))
        .with_state(AppState {
            data: Arc::new(data),
            shutdown: shutdown_signal,
        })
        .layer(CorsLayer::permissive());

    match assets_override.filter(|p| p.join("index.html").is_file()) {
        Some(dist) => {
            let index = dist.join("index.html");
            let files = ServeDir::new(&dist).not_found_service(ServeFile::new(index));
            api.fallback_service(files)
        }
        None if assets::HAVE => api.fallback(embedded),
        None => api.fallback(no_assets),
    }
}

/// Serve `data` on `127.0.0.1:port` (port 0 picks a free port), optionally
/// opening the browser, and block until Ctrl-C or the report's stop button. Builds its own tokio runtime so
/// it can be called from a plain (non-async) context, e.g. the Python binding.
pub fn run_server(
    data: ReportData,
    port: u16,
    open: bool,
    assets_override: Option<PathBuf>,
) -> Result<(), DataError> {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    rt.block_on(async move {
        let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
        let listener = tokio::net::TcpListener::bind(addr).await?;
        let url = format!("http://{}", listener.local_addr()?);
        println!("qweave report: {url}  (Ctrl-C to stop)");
        if open {
            open_browser(&url);
        }
        let shutdown = Arc::new(tokio::sync::Notify::new());
        axum::serve(
            listener,
            router_with_shutdown(data, assets_override, shutdown.clone()),
        )
        .with_graceful_shutdown(async move {
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {}
                _ = shutdown.notified() => {}
            }
        })
        .await
    })?;
    Ok(())
}

async fn meta(State(state): State<AppState>) -> Json<Value> {
    Json(state.data.meta_value())
}

async fn summary(State(state): State<AppState>) -> Json<Value> {
    Json(state.data.summary_records())
}

async fn factor(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<Json<Value>, ApiError> {
    Ok(Json(state.data.factor_bundle(&name)?))
}

async fn shutdown(State(state): State<AppState>) -> StatusCode {
    state.shutdown.notify_one();
    StatusCode::OK
}

/// Serve an embedded frontend file, falling back to `index.html` for SPA routes.
async fn embedded(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    let path = if path.is_empty() { "index.html" } else { path };
    match assets::get(path) {
        Some(bytes) => ([(header::CONTENT_TYPE, content_type(path))], bytes).into_response(),
        None => (
            [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
            assets::get("index.html").unwrap_or_default(),
        )
            .into_response(),
    }
}

async fn no_assets() -> impl IntoResponse {
    (
        StatusCode::OK,
        "qweave report: API is up at /api/*. Build the frontend \
         (cd frontend && npm run build) and rebuild to serve the UI.",
    )
}

fn content_type(path: &str) -> &'static str {
    match path.rsplit('.').next() {
        Some("html") => "text/html; charset=utf-8",
        Some("js") => "text/javascript",
        Some("css") => "text/css",
        Some("json") | Some("map") => "application/json",
        Some("svg") => "image/svg+xml",
        Some("ico") => "image/x-icon",
        Some("woff2") => "font/woff2",
        _ => "application/octet-stream",
    }
}

/// Best-effort launch of the default browser; failures are non-fatal.
fn open_browser(url: &str) {
    #[cfg(target_os = "windows")]
    let (cmd, args) = ("cmd", vec!["/C", "start", "", url]);
    #[cfg(target_os = "macos")]
    let (cmd, args) = ("open", vec![url]);
    #[cfg(all(unix, not(target_os = "macos")))]
    let (cmd, args) = ("xdg-open", vec![url]);

    let _ = std::process::Command::new(cmd).args(args).spawn();
}

/// API error → HTTP status. A missing table/factor is a 404; anything else is a
/// 500 with the polars/IO message (this is a localhost dev tool).
struct ApiError(StatusCode, String);

impl From<DataError> for ApiError {
    fn from(err: DataError) -> Self {
        let status = match err {
            DataError::Missing(_) => StatusCode::NOT_FOUND,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };
        ApiError(status, err.to_string())
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.0, Json(json!({ "error": self.1 }))).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use http_body_util::BodyExt;
    use polars::prelude::*;
    use qweave_core::PanelOptions;
    use qweave_eval::{Binning, Demean, EvaluateOptions, Weighting, evaluate};
    use tower::ServiceExt;

    fn fixture_dir() -> PathBuf {
        let mut df = df!(
            "asset" => ["A", "B", "C", "D", "A", "B", "C", "D"],
            "time" => [20481i32, 20481, 20481, 20481, 20512, 20512, 20512, 20512],
            "f1" => [1.0f64, 2.0, 3.0, 4.0, 4.0, 3.0, 2.0, 1.0],
            "ret_1" => [0.01f64, 0.02, 0.03, 0.04, 0.04, 0.03, 0.02, 0.01],
        )
        .unwrap();
        let time = df.column("time").unwrap().cast(&DataType::Date).unwrap();
        df.with_column(time).unwrap();

        let panel = PanelOptions {
            symbol_col: "asset".into(),
            time_col: "time".into(),
        };
        let dir = std::env::temp_dir().join(format!("qweave-server-test-{}", std::process::id()));
        let opts = EvaluateOptions {
            factor_cols: vec!["f1".into()],
            label_cols: None,
            quantiles: 2,
            binning: Binning::Daily,
            demean: Demean::None,
            min_cs_count: 2,
            group_col: None,
            tradable_col: None,
            cost_bps: 0.0,
            weighting: Weighting::Factor,
            factor_source: None,
            output_dir: Some(dir.to_string_lossy().into_owned()),
        };
        evaluate(&df, &panel, &opts).unwrap();
        dir
    }

    async fn get_json(app: &Router, uri: &str) -> (StatusCode, Value) {
        let res = app
            .clone()
            .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
            .await
            .unwrap();
        let status = res.status();
        let bytes = res.into_body().collect().await.unwrap().to_bytes();
        let value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        (status, value)
    }

    #[tokio::test]
    async fn serves_meta_summary_and_factor() {
        let dir = fixture_dir();
        let app = router(ReportData::from_dir(&dir).unwrap(), None);

        let (status, meta) = get_json(&app, "/api/meta").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(meta["factors"], json!(["f1"]));
        assert_eq!(meta["quantiles"], json!(2));

        let (status, summary) = get_json(&app, "/api/summary").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(summary.as_array().unwrap()[0]["factor"], json!("f1"));

        let (status, bundle) = get_json(&app, "/api/factor/f1").await;
        assert_eq!(status, StatusCode::OK);
        assert!(bundle["ic"]["ic"].as_array().unwrap().len() >= 2);
        assert!(bundle["ic"]["date"][0].as_str().unwrap().contains('-'));
        assert!(bundle["quantiles"]["mean_ret_1"].is_array());
        assert!(bundle["monthly"].is_object());

        let (status, _) = get_json(&app, "/api/factor/nope").await;
        assert_eq!(status, StatusCode::NOT_FOUND);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn shutdown_endpoint_notifies_server() {
        let dir = fixture_dir();
        let signal = Arc::new(tokio::sync::Notify::new());
        let app = router_with_shutdown(ReportData::from_dir(&dir).unwrap(), None, signal.clone());
        let response = app
            .oneshot(Request::post("/api/shutdown").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        tokio::time::timeout(std::time::Duration::from_secs(1), signal.notified())
            .await
            .expect("shutdown signal");
        std::fs::remove_dir_all(&dir).ok();
    }
}
