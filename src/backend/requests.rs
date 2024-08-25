use crate::{backend::*, error, search::*};
use either::Either;
use endpoints::chat::*;
use hyper::{Body, Request, Response};
use llama_core::search::*;

/// Simply retrun whether the query requires an internet search.
pub(crate) async fn query_handler(
    req: Request<Body>,
    cli: &crate::Cli,
    query_type: crate::backend::QueryType,
) -> Response<Body> {
    info!(target: "query_handler", "Handling the incoming decision request.");

    let bytes = match hyper::body::to_bytes(req.into_body()).await {
        Ok(bytes) => bytes,
        Err(e) => {
            let msg = format!(
                "Error while converting request body into bytes: {}\n",
                e.to_string()
            );
            error!(target: "query_handler", "{}", msg);
            return error::internal_server_error(msg);
        }
    };
    let bytes_json: serde_json::Value = match serde_json::from_slice(&bytes) {
        Ok(bytes_json) => bytes_json,
        Err(e) => {
            let msg = format!(
                "Error while converting request body into json object: {}",
                e.to_string()
            );
            error!(target: "query_handler", "{}", msg);
            return error::internal_server_error(msg);
        }
    };
    let request_search_config = match bytes_json.get("search_config") {
        Some(object) => object,
        None => {
            let msg = "Unable to extract search_config object from request.\n";
            error!(target: "query_handler", "{}", msg);
            return error::internal_server_error(msg);
        }
    };
    let search_backend =
        SearchBackends::from(bytes_json["backend"].as_str().unwrap_or("").to_string());

    if cli.server {
        if search_backend == SearchBackends::LocalSearchServer {
            let msg = "The \"backend local_search_server\" is only allowed on servers configured without --server.\n";
            error!(target: "query_handler", "{}", msg);
            return error::bad_request(msg);
        }
        if query_type == QueryType::Summarize {
            let msg =
                "Summary generation endpoint is only available on servers configured without --server.\n";
            error!(target: "query_handler", "{}", msg);
            return error::bad_request(msg);
        }
    }

    // set the search backend according the user's requirement.
    let search_config = match search_backend {
        SearchBackends::Tavily => SearchConfig {
            search_engine: "tavily".to_string(),
            max_search_results: request_search_config["max_search_results"]
                .as_u64()
                .unwrap_or(cli.max_search_results as u64)
                .min(u8::max as u64) as u8,
            size_limit_per_result: request_search_config["size_limit_per_result"]
                .as_u64()
                .unwrap_or(cli.size_per_search_result as u64)
                .min(u16::max as u64) as u16,
            endpoint: "https://api.tavily.com/search".to_owned(),
            content_type: ContentType::JSON,
            output_content_type: ContentType::JSON,
            method: "POST".to_string(),
            additional_headers: None,
            parser: tavily_search::tavily_parser,
            summarization_prompts: None,
            summarize_ctx_size: None,
        },
        SearchBackends::Bing => {
            // Bing Web Search API expects the api key in request headers.
            let mut additional_headers = std::collections::HashMap::new();
            let api_key = match request_search_config.get("api_key") {
                Some(api_key) => match api_key.as_str() {
                    Some(key) => key,
                    None => {
                        let msg = "invalid Bing API key supplied.\n";
                        error!(target:"query_handler", "{}", msg);
                        return error::internal_server_error(msg);
                    }
                },
                None => {
                    let msg = "no Bing API key supplied.\n";
                    error!(target:"query_handler", "{}", msg);
                    return error::bad_request(msg);
                }
            };
            additional_headers.insert("Ocp-Apim-Subscription-Key".to_string(), api_key.to_string());

            SearchConfig {
                search_engine: "bing".to_string(),
                max_search_results: request_search_config
                    .get("max_search_results")
                    .unwrap_or(&serde_json::Value::from(u64::MAX))
                    .as_u64()
                    .unwrap_or(cli.max_search_results as u64)
                    .min(u8::MAX as u64) as u8,
                size_limit_per_result: request_search_config
                    .get("size_limit_per_result")
                    .unwrap_or(&serde_json::Value::from(u64::MAX))
                    .as_u64()
                    .unwrap_or(cli.size_per_search_result as u64)
                    .min(u16::MAX as u64) as u16,
                endpoint: "https://api.bing.microsoft.com/v7.0/search".to_owned(),
                content_type: ContentType::JSON,
                output_content_type: ContentType::JSON,
                method: "GET".to_string(),
                additional_headers: Some(additional_headers),
                parser: bing_search::bing_parser,
                summarization_prompts: None,
                summarize_ctx_size: None,
            }
        }
        SearchBackends::LocalSearchServer => SearchConfig {
            search_engine: "google".to_string(),
            max_search_results: request_search_config
                .get("max_search_results")
                .unwrap_or(&serde_json::Value::from(u64::MAX))
                .as_u64()
                .unwrap_or(cli.max_search_results as u64)
                .min(u8::MAX as u64) as u8,
            size_limit_per_result: request_search_config
                .get("size_limit_per_result")
                .unwrap_or(&serde_json::Value::from(u64::MAX))
                .as_u64()
                .unwrap_or(cli.size_per_search_result as u64)
                .min(u16::MAX as u64) as u16,
            endpoint: request_search_config
                .get("endpoint")
                .unwrap_or(&serde_json::Value::from("https://localhost:3000/search"))
                .as_str()
                .unwrap()
                .to_string(),
            content_type: ContentType::JSON,
            output_content_type: ContentType::JSON,
            method: "POST".to_string(),
            additional_headers: None,
            parser: bing_search::bing_parser,
            summarization_prompts: None,
            summarize_ctx_size: None,
        },
        SearchBackends::Unknown => {
            let msg = "Unknown backend mentioned.\nUsage: tavily, bing, local_search_server.\n";
            error!(target: "query_handler", "{}", msg);
            return error::bad_request(msg);
        }
    };
    let query = match bytes_json.get("query") {
        Some(query) => match query.as_str() {
            Some(q) => q.to_string(),
            None => {
                let msg = "The query supplied is not a String.\n";
                error!(target:"query_handler", "{}", msg);
                return error::internal_server_error(msg);
            }
        },
        None => {
            let msg = "No query received.\n";
            error!(target:"query_handler", "{}", msg);
            return error::bad_request(msg);
        }
    };

    // extract query
    let consultation_response: ConsultResponse = match consult(query, cli.model_name.clone()).await
    {
        Ok(cr) => cr,
        Err(_) => {
            let msg = "Error while generating response from LLM.\n";
            error!(target: "query_handler", "{}", msg);
            return error::internal_server_error(msg);
        }
    };

    //the response body
    let body: String;

    // generate the actual response body based on the requested level of response detail.
    match query_type {
        // just make a decision on whether to search the internet and return the needed query.
        QueryType::Decision => {
            body = (serde_json::json!({
                "decision": consultation_response.decision.clone(),
                "query": consultation_response.query.unwrap_or("null".to_string())
            }))
            .to_string();
        }
        // make a decision and perform the internet search if needed
        QueryType::Complete => {
            let computed_query = consultation_response
                .query
                .clone()
                .unwrap_or("".to_string());
            let search_output: SearchOutput;

            if !consultation_response.decision {
                body = (serde_json::json!({
                    "decision": consultation_response.decision.clone(),
                    "query": consultation_response.query.clone().unwrap_or("null".to_string())
                }))
                .to_string();
            } else {
                match search_backend {
                    SearchBackends::Tavily => {
                        let search_input = tavily_search::TavilySearchInput {
                            api_key: match request_search_config.get("api_key") {
                                Some(api_key) => match api_key.as_str() {
                                    Some(key) => key.to_string(),
                                    None => {
                                        let msg = "Invalid Tavily API key supplied.\n";
                                        error!(target:"query_handler", "{}", msg);
                                        return error::bad_request(msg);
                                    }
                                },
                                None => {
                                    let msg = "no Tavily API key supplied.\n";
                                    error!(target:"query_handler", "{}", msg);
                                    return error::internal_server_error(msg);
                                }
                            },
                            include_answer: false,
                            include_images: false,
                            query: computed_query,
                            max_results: search_config.max_search_results,
                            include_raw_content: false,
                            search_depth: "advanced".to_string(),
                        };
                        search_output = match search_config.perform_search(&search_input).await {
                            Ok(so) => so,
                            Err(e) => {
                                return error::internal_server_error(format!(
                                    "Failed to perform internet search: {}",
                                    e.to_string()
                                ));
                            }
                        };
                    }
                    SearchBackends::Bing => {
                        let search_input = bing_search::BingSearchInput {
                            count: search_config.max_search_results,
                            q: computed_query,
                            responseFilter: "Webpages".to_string(),
                        };
                        search_output = match search_config.perform_search(&search_input).await {
                            Ok(so) => so,
                            Err(e) => {
                                return error::internal_server_error(format!(
                                    "Failed to perform internet search: {}",
                                    e.to_string()
                                ));
                            }
                        };
                    }
                    SearchBackends::LocalSearchServer => {
                        let search_input = local_google_search::LocalGoogleSearchInput {
                            term: computed_query,
                            engine: "google".to_string(),
                            maxSearchResults: search_config.max_search_results,
                        };
                        search_output = match search_config.perform_search(&search_input).await {
                            Ok(so) => so,
                            Err(e) => {
                                return error::internal_server_error(format!(
                                    "Failed to perform internet search: {}",
                                    e.to_string()
                                ));
                            }
                        };
                    }
                    SearchBackends::Unknown => {
                        let msg =
                            "Unknown backend mentioned.\nUsage: tavily, bing, local_search_server\n"
                                .to_string();
                        error!(target: "query_handler", "{}", msg);
                        return error::bad_request(msg);
                    }
                };

                body = (serde_json::json!({
                    "decision": consultation_response.decision.clone(),
                    "results": search_output.results
                }))
                .to_string();
            }
        }
        // make a decision and perform the internet search if needed
        QueryType::Summarize => {
            let computed_query = consultation_response
                .query
                .clone()
                .unwrap_or("".to_string());
            let search_output: String;

            if !consultation_response.decision {
                body = (serde_json::json!({
                    "decision": consultation_response.decision.clone(),
                    "results": consultation_response.query.clone().unwrap_or("null".to_string())
                }))
                .to_string();
            } else {
                match search_backend {
                    SearchBackends::Tavily => {
                        let search_input = tavily_search::TavilySearchInput {
                            api_key: match request_search_config.get("api_key") {
                                Some(api_key) => match api_key.as_str() {
                                    Some(key) => key.to_string(),
                                    None => {
                                        let msg = "Invalid Tavily API key supplied.\n";
                                        error!(target:"query_handler", "{}", msg);
                                        return error::bad_request(msg);
                                    }
                                },
                                None => {
                                    let msg = "No Tavily API key supplied\n";
                                    error!(target:"query_handler", "{}", msg);
                                    return error::internal_server_error(msg);
                                }
                            },
                            include_answer: false,
                            include_images: false,
                            query: computed_query,
                            max_results: search_config.max_search_results,
                            include_raw_content: false,
                            search_depth: "advanced".to_string(),
                        };
                        search_output = match search_config.summarize_search(&search_input).await {
                            Ok(so) => so,
                            Err(e) => {
                                return error::internal_server_error(format!(
                                    "Failed to perform internet search: {}",
                                    e.to_string()
                                ));
                            }
                        };
                    }
                    SearchBackends::Bing => {
                        let search_input = bing_search::BingSearchInput {
                            count: search_config.max_search_results,
                            q: computed_query,
                            responseFilter: "Webpages".to_string(),
                        };
                        search_output = match search_config.summarize_search(&search_input).await {
                            Ok(so) => so,
                            Err(e) => {
                                return error::internal_server_error(format!(
                                    "Failed to perform internet search: {}",
                                    e.to_string()
                                ));
                            }
                        };
                    }
                    SearchBackends::LocalSearchServer => {
                        let search_input = local_google_search::LocalGoogleSearchInput {
                            term: computed_query,
                            engine: "google".to_string(),
                            maxSearchResults: search_config.max_search_results,
                        };
                        search_output = match search_config.summarize_search(&search_input).await {
                            Ok(so) => so,
                            Err(e) => {
                                return error::internal_server_error(format!(
                                    "Failed to perform internet search: {}",
                                    e.to_string()
                                ));
                            }
                        };
                    }
                    SearchBackends::Unknown => {
                        let msg =
                            "unknown backend mentioned.\nUsage: tavily, bing, local_search_server"
                                .to_string();
                        error!(target: "query_handler", "{}", msg);
                        return error::bad_request(msg);
                    }
                };

                body = (serde_json::json!({
                    "decision": consultation_response.decision.clone(),
                    "results": search_output
                }))
                .to_string();
            }
        }
    };

    let result = Response::builder()
        .header("Access-Control-Allow-Origin", "*")
        .header("Access-Control-Allow-Methods", "*")
        .header("Access-Control-Allow-Headers", "*")
        .header("Content-Type", "application/json")
        .body(Body::from(body));

    let res = match result {
        Ok(response) => response,
        Err(e) => {
            let err_msg = format!("failed to build a response. Reason: {}", e);

            // log
            error!(target: "query_handler", "{}", &err_msg);

            error::internal_server_error(err_msg)
        }
    };

    // log
    info!(target: "query_handler", "Replying to consultation.");

    res
}

/// Consult the LLM (generate a Tool Call) to decide whether the query requires an internet search
///
/// Will return an Option<String>
async fn consult(query: String, model_name: String) -> Result<ConsultResponse, error::ServerError> {
    let mut messages: Vec<ChatCompletionRequestMessage> = Vec::new();

    // create a system message
    let system_message = ChatCompletionRequestMessage::System(ChatCompletionSystemMessage::new(
            r##"
            You are an intent classification model. Your goal is to determine whether a given user query can only be answered with additional information from a google search. Use the search_required tool call for this.

            Instructions:

                For each query, assign an appropriate intent:
                    `true` if the query would typically need real-time data, specific information retrieval, or content that is not likely to be pre-known. Generate the search term for the query.
                    `false` if the query can be answered based on general knowledge, static facts, or content that can be reasonably assumed to be within the model's scope."##.to_string(),
        None,
    ));

    messages.push(system_message);

    //create a user message
    let user_message = ChatCompletionRequestMessage::User(ChatCompletionUserMessage::new(
        ChatCompletionUserMessageContent::Text(query),
        None,
    ));

    messages.push(user_message);

    // Web Search tool parameters
    let search_required_params = ToolFunctionParameters {
        schema_type: JSONSchemaType::Object,
        properties: Some(
            vec![
                (
                    "search_required".to_string(),
                    Box::new(JSONSchemaDefine {
                        schema_type: Some(JSONSchemaType::Boolean),
                        description: Some(
                            "Whether an internet search is required to answer the query."
                                .to_string(),
                        ),
                        enum_values: None,
                        properties: None,
                        required: None,
                        items: None,
                    }),
                ),
                (
                    "query".to_string(),
                    Box::new(JSONSchemaDefine {
                        schema_type: Some(JSONSchemaType::Boolean),
                        description: Some("The query to search if search is required.".to_string()),
                        enum_values: None,
                        properties: None,
                        required: None,
                        items: None,
                    }),
                ),
            ]
            .into_iter()
            .collect(),
        ),
        required: Some(vec!["search_required".to_string()]),
    };
    // Web Search tool
    let search_required = Tool {
        ty: "function".to_string(),
        function: ToolFunction {
            name: "search_required".to_string(),
            description: Some("Use to search the internet to answer a query.".to_string()),
            parameters: Some(search_required_params),
        },
    };

    // create a chat completion request
    let mut request = ChatCompletionRequestBuilder::new(model_name, messages)
        // no stream required.
        .enable_stream(false)
        .with_n_choices(1)
        .with_max_tokens(500)
        .with_reponse_format(ChatResponseFormat::default())
        .with_tools(vec![search_required])
        .with_tool_choice(ToolChoice::Tool(ToolChoiceTool {
            ty: "function".to_string(),
            function: ToolChoiceToolFunction {
                name: "search_required".to_string(),
            },
        }))
        .build();

    // serlialize and log input
    info!(target: "consult", "search request: \n\n{:?}\n", request);

    let consultation_result: ChatCompletionObject = match llama_core::chat::chat(&mut request).await
    {
        Ok(result) => {
            match result {
                Either::Right(chat_completion_object) => {
                    // serialize chat completion object
                    let consultation_result =
                        serde_json::to_string(&chat_completion_object).unwrap();
                    info!(target: "consult", "consultation_result: \n\n{}\n", consultation_result);
                    chat_completion_object
                }
                Either::Left(_) => {
                    let msg = "streaming mode is unsupported".to_string();
                    error!(target: "consult", "{}", msg);
                    return Err(error::ServerError::ConsulationError(msg));
                }
            }
        }
        Err(e) => {
            let msg = e.to_string();
            error!(target: "consult", "{}", msg);
            return Err(error::ServerError::ConsulationError(msg));
        }
    };

    println!("{:?}", consultation_result);
    // extract and validate tool call. There should only be one system call (one query => one call)
    let tool_call: ToolCall = consultation_result.choices[0].message.tool_calls[0].clone();

    if tool_call.ty != "function" || tool_call.function.name != "search_required" {
        let msg = format!("Invalid tool call response:\n\n{:?}\n", tool_call);
        error!(target: "consult", "{}", msg);
        return Err(error::ServerError::ConsulationError(msg));
    }

    let arguments: serde_json::Value =
        match serde_json::from_str(tool_call.function.arguments.as_str()) {
            Ok(v) => v,
            Err(_) => {
                let msg = "Could not deserialize tool call arguments".to_string();
                error!(target: "consult", "{}", msg);
                return Err(error::ServerError::ConsulationError(msg));
            }
        };
    if !arguments["search_required"].is_boolean() {
        let msg = format!("Invalid argument type: search_required");
        error!(target: "consult", "{}", msg);
        return Err(error::ServerError::ConsulationError(msg));
    }
    if arguments["search_required"].as_bool().unwrap() && arguments["query"].is_null() {
        let msg = "invalid argument: 'query' cannot be null".to_string();
        error!(target: "consult", "{}", msg);
        return Err(error::ServerError::ConsulationError(msg));
    }

    // tool call validated. build and return ConsultResponse.
    Ok(ConsultResponse {
        decision: arguments["search_required"].as_bool().unwrap(),

        query: if arguments["search_required"].as_bool().unwrap() {
            Some(arguments["query"].as_str().unwrap().to_string())
        } else {
            None
        },
    })
}

/// Reason for a decision
// enum Reason {
//     FollowUp,
//     NotRequired,
// }

/// The response from the LLM, cleaned
struct ConsultResponse {
    pub decision: bool,
    pub query: Option<String>,
}
