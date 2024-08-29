#[macro_use]
extern crate log;

mod backend;
mod error;
mod search;
mod utils;

use crate::error::ServerError;
use anyhow::Result;
use chat_prompts::PromptTemplateType;
use clap::Parser;
use hyper::{
    body::HttpBody,
    server::conn::AddrStream,
    service::{make_service_fn, service_fn},
    Body, Request, Response, Server,
};
use llama_core::MetadataBuilder;
use once_cell::sync::OnceCell;
use std::path::PathBuf;
use tokio::net::TcpListener;
use utils::LogLevel;

type Error = Box<dyn std::error::Error + Send + Sync + 'static>;

const DEFAULT_SOCKET_ADDRESS: &str = "0.0.0.0:8081";
// To make the CLI accessible from the request functions, as it cannot implement the "Copy" trait
// required for the `async move`
pub(crate) static CLI: OnceCell<Cli> = OnceCell::new();

#[derive(Debug, Parser)]
#[command(name = "LlamaEdge-Search API Server", version = env!("CARGO_PKG_VERSION"), author = env!("CARGO_PKG_AUTHORS"), about = "LlamaEdge-Search API Server")]
struct Cli {
    /// Sets names for chat model.
    #[arg(short, long, default_value = "default")]
    model_name: String,
    /// Model aliases for chat model.
    #[arg(short = 'a', long, default_value = "default")]
    model_alias: String,
    /// Sets context sizes for the chat model.
    #[arg(short = 'c', long, default_value = "1024")]
    ctx_size: u64,
    /// Sets batch sizes for the chat model.
    #[arg(short, long, value_delimiter = ',', default_value = "512,512", value_parser = clap::value_parser!(u64))]
    batch_size: u64,
    /// Sets prompt templates for the chat model.
    #[arg(short, long, value_delimiter = ',', value_parser = clap::value_parser!(PromptTemplateType), required = true)]
    prompt_template: PromptTemplateType,
    /// Halt generation at PROMPT, return control.
    #[arg(short, long)]
    reverse_prompt: Option<String>,
    /// Number of tokens to predict
    #[arg(short, long, default_value = "1024")]
    n_predict: u64,
    /// Number of layers to run on the GPU
    #[arg(short = 'g', long, default_value = "100")]
    n_gpu_layers: u64,
    /// Disable memory mapping for file access of chat models
    #[arg(long)]
    no_mmap: Option<bool>,
    /// Temperature for sampling
    #[arg(long, default_value = "0.0")]
    temp: f64,
    /// An alternative to sampling with temperature, called nucleus sampling, where the model considers the results of the tokens with top_p probability mass. 1.0 = disabled
    #[arg(long, default_value = "1.0")]
    top_p: f64,
    /// Penalize repeat sequence of tokens
    #[arg(long, default_value = "1.1")]
    repeat_penalty: f64,
    /// Repeat alpha presence penalty. 0.0 = disabled
    #[arg(long, default_value = "0.0")]
    presence_penalty: f64,
    /// Repeat alpha frequency penalty. 0.0 = disabled
    #[arg(long, default_value = "0.0")]
    frequency_penalty: f64,
    /// Path to the multimodal projector file
    #[arg(long)]
    llava_mmproj: Option<String>,
    /// Socket address of LlamaEdge API Server instance
    #[arg(long, default_value = DEFAULT_SOCKET_ADDRESS)]
    socket_addr: String,
    /// Deprecated. Print prompt strings to stdout
    #[arg(long)]
    log_prompts: bool,
    /// Deprecated. Print statistics to stdout
    #[arg(long)]
    log_stat: bool,
    /// Deprecated. Print all log information to stdout
    #[arg(long)]
    log_all: bool,
    /// Fallback: Maximum search results to be enforced in case a user query goes overboard.
    #[arg(long, default_value = "5")]
    max_search_results: u8,
    /// Fallback: Size limit per result to be enforced in case a user query goes overboard.
    #[arg(long, default_value = "400")]
    size_per_search_result: u16,
    /// Whether the server is running locally on a user's machine. enables local-search-server
    /// usage and summariztion.
    #[arg(long, default_value = "false")]
    server: bool,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), ServerError> {
    // get the environment variable `RUST_LOG`
    let rust_log = std::env::var("RUST_LOG").unwrap_or_default().to_lowercase();
    let (_, log_level) = match rust_log.is_empty() {
        true => ("stdout", LogLevel::Info),
        false => match rust_log.split_once("=") {
            Some((target, level)) => (target, level.parse().unwrap_or(LogLevel::Info)),
            None => ("stdout", rust_log.parse().unwrap_or(LogLevel::Info)),
        },
    };

    // set global logger
    wasi_logger::Logger::install().expect("failed to install wasi_logger::Logger");
    log::set_max_level(log_level.into());

    // parse the commandline arguments
    let cli = Cli::parse();

    // number of tokens to predict
    info!("[INFO] Number of tokens to predict: {n}", n = cli.n_predict);

    // n_gpu_layers
    info!(
        "[INFO] Number of layers to run on the GPU: {n}",
        n = cli.n_gpu_layers
    );

    // no_mmap
    if cli.no_mmap.is_some() {
        println!("[INFO] no mmap: {nommap}", nommap = !cli.no_mmap.unwrap());
    }
    // batch size
    info!(
        "[INFO] Batch size for prompt processing: {size}",
        size = &cli.batch_size
    );

    // reverse_prompt
    if let Some(reverse_prompt) = &cli.reverse_prompt {
        println!("[INFO] Reverse prompt: {prompt}", prompt = &reverse_prompt);
    }

    // log
    let log_enable = cli.log_all;
    println!("[INFO] Log enable: {enable}", enable = log_enable);

    let metadata_chat = MetadataBuilder::new(
        cli.model_name.clone(),
        cli.model_alias.clone(),
        cli.prompt_template,
    )
    .with_ctx_size(cli.ctx_size)
    .with_batch_size(cli.batch_size)
    .with_n_predict(cli.n_predict)
    .with_n_gpu_layers(cli.n_gpu_layers)
    .disable_mmap(cli.no_mmap)
    .with_temperature(cli.temp)
    .with_top_p(cli.top_p)
    .with_repeat_penalty(cli.repeat_penalty)
    .with_presence_penalty(cli.presence_penalty)
    .with_frequency_penalty(cli.frequency_penalty)
    .with_reverse_prompt(cli.reverse_prompt.clone())
    .with_mmproj(cli.llava_mmproj.clone())
    .enable_plugin_log(true)
    .enable_debug_log(true)
    .build();
    // initialize the core context
    if let Err(e) = llama_core::init_core_context(Some(&[metadata_chat]), None) {
        let msg = format!("Failed to initialize core context: {}", e.to_string());
        error!(target: "stdout", "{}", msg);
        return Err(error::ServerError::Operation(msg));
    }

    // socket address
    let addr = cli
        .socket_addr
        .parse::<std::net::SocketAddr>()
        .map_err(|e| ServerError::SocketAddr(e.to_string()))?;

    CLI.set(cli)
        .map_err(|_| ServerError::Operation("Failed to set `CLI`.".to_owned()))?;
    // log socket address
    info!(target: "stdout", "socket_address: {}", addr.to_string());

    let new_service = make_service_fn(move |conn: &AddrStream| {
        // log socket address
        info!(target: "stdout", "remote_addr: {}, local_addr: {}", conn.remote_addr().to_string(), conn.local_addr().to_string());

        async move { Ok::<_, Error>(service_fn(move |req| handle_request(req))) }
    });

    let tcp_listener = TcpListener::bind(addr).await.unwrap();
    let server = Server::from_tcp(tcp_listener.into_std().unwrap())
        .unwrap()
        .serve(new_service);
    //let server = Server::bind(&addr).serve(new_service);

    match server.await {
        Ok(_) => Ok(()),
        Err(e) => Err(ServerError::Operation(e.to_string())),
    }
}

async fn handle_request(req: Request<Body>) -> Result<Response<Body>, hyper::Error> {
    let cli = match CLI.get() {
        Some(cli) => cli,
        None => {
            let msg = "Failed to obtain SEARCH_CONFIG. Was it set?".to_string();
            error!(target: "stdout", "{}", &msg);

            return Ok(error::internal_server_error(msg));
        }
    };
    let path_str = req.uri().path();
    let path_buf = PathBuf::from(path_str);
    let mut path_iter = path_buf.iter();
    path_iter.next(); // Must be Some(OsStr::new(&path::MAIN_SEPARATOR.to_string()))
    let root_path = path_iter.next().unwrap_or_default();
    let root_path = "/".to_owned() + root_path.to_str().unwrap_or_default();

    // log request
    {
        let method = hyper::http::Method::as_str(req.method()).to_string();
        let path = req.uri().path().to_string();
        let version = format!("{:?}", req.version());
        if req.method() == hyper::http::Method::POST {
            let size: u64 = req
                .headers()
                .get("content-length")
                .unwrap()
                .to_str()
                .unwrap()
                .parse()
                .unwrap();

            info!(target: "stdout", "method: {}, endpoint: {}, http_version: {}, size: {}", method, path, version, size);
        } else {
            info!(target: "stdout", "method: {}, endpoint: {}, http_version: {}", method, path, version);
        }
    }

    let response = match root_path.as_str() {
        "/echo" => Response::new(Body::from("echo test")),
        "/query" => backend::handle_query_request(req, &cli).await,
        _ => error::not_implemented(),
    };

    // log response
    {
        let status_code = response.status();
        if status_code.as_u16() < 400 {
            // log response
            let response_version = format!("{:?}", response.version());
            let response_body_size: u64 = response.body().size_hint().lower();
            let response_status = status_code.as_u16();
            let response_is_informational = status_code.is_informational();
            let response_is_success = status_code.is_success();
            let response_is_redirection = status_code.is_redirection();
            let response_is_client_error = status_code.is_client_error();
            let response_is_server_error = status_code.is_server_error();

            info!(target: "stdout", "version: {}, body_size: {}, status: {}, is_informational: {}, is_success: {}, is_redirection: {}, is_client_error: {}, is_server_error: {}", response_version, response_body_size, response_status, response_is_informational, response_is_success, response_is_redirection, response_is_client_error, response_is_server_error);
        } else {
            let response_version = format!("{:?}", response.version());
            let response_body_size: u64 = response.body().size_hint().lower();
            let response_status = status_code.as_u16();
            let response_is_informational = status_code.is_informational();
            let response_is_success = status_code.is_success();
            let response_is_redirection = status_code.is_redirection();
            let response_is_client_error = status_code.is_client_error();
            let response_is_server_error = status_code.is_server_error();

            error!(target: "stdout", "version: {}, body_size: {}, status: {}, is_informational: {}, is_success: {}, is_redirection: {}, is_client_error: {}, is_server_error: {}", response_version, response_body_size, response_status, response_is_informational, response_is_success, response_is_redirection, response_is_client_error, response_is_server_error);
        }
    }

    Ok(response)
}
