mod requests;

use crate::error;
use hyper::{Body, Request, Response};

#[derive(PartialEq)]
pub(crate) enum QueryType {
    Decision,
    Complete,
    Summarize,
}

pub(crate) async fn handle_query_request(req: Request<Body>, cli: &crate::Cli) -> Response<Body> {
    match req.uri().path() {
        "/query/decide" => requests::query_handler(req, cli, QueryType::Decision).await,
        "/query/complete" => requests::query_handler(req, cli, QueryType::Complete).await,
        "/query/summarize" => requests::query_handler(req, cli, QueryType::Summarize).await,
        _ => error::not_implemented(),
    }
}
