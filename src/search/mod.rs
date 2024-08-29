pub mod bing_search;
pub mod tavily_search;

#[derive(PartialEq)]
pub(crate) enum SearchBackends {
    Tavily,
    Bing,
    Unknown,
}

// Implementing for String and not str to make it eaiser to use when comparing using JSON fields.
impl From<std::string::String> for SearchBackends {
    fn from(search_backend: String) -> Self {
        match search_backend.as_str() {
            "tavily" => Self::Tavily,
            "bing" => Self::Bing,
            _ => Self::Unknown,
        }
    }
}
