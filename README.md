# LlamaEdge Query Server
<!-- @import "[TOC]" {cmd="toc" depthFrom=1 depthTo=6 orderedList=false} -->

The LlamaEdge Query Server allows the user to check whether a chatbot query requires an internet search to answer. Additionally, it can also complete these searches as well as perform summarization on them (in local mode).

There are 3 endpoints: `decide`, `complete`, `summarize`

The decision endpoint: `POST /query/decide`
- Consults the LLM about whether the `query` passed in requires an internet search, and return `true` or `false` along with .

<details> <summary> Example </summary>

Input
```bash
curl -k "http://0.0.0.0:8080/query/decide" -d '{"search_config":{}, "backend":"tavily", "query": "Whats the capital of france"}'
```

Output:
```json
{
  "decision": true,
  "query": "What is the capital of France"
}
```

</details>

The complete endpoint: `POST /query/complete`
- If the decision by the LLM is `true`, then it also performs the internet search according to the given `search_config` sent to the LLM.

<details> <summary> Example </summary>

Input
```bash
curl -k "http://0.0.0.0:8080/query/complete" -d '{"search_config":{"api_key":"xxx"}, "backend":"tavily", "query": "Whats the capital of france"}'
```

Output:
```json
{
  "decision": true,
  "results": [
    {
      "site_name": "Paris Facts | Britannica",
      "text_content": "Paris is the capital of France, located in the north-central part of the country. It is a",
      "url": "https://www.britannica.com/facts/Paris"
    },
    {
      "site_name": "Capital of France - Simple English Wikipedia, the free encyclopedia",
      "text_content": "Learn about the history and current status of the capital of France, which is Paris. Find",
      "url": "https://simple.wikipedia.org/wiki/Capital_of_France"
    },
    {
      "site_name": "Paris - Simple English Wikipedia, the free encyclopedia",
      "text_content": "Events[change | change source]\nRelated pages[change | change source]\nReferences[change | ",
      "url": "https://simple.wikipedia.org/wiki/Paris"
    },
    {
      "site_name": "What is the Capital of France? - WorldAtlas",
      "text_content": "Geography and Climate\nLocated in the north of Central France, the city is relatively flat",
      "url": "https://www.worldatlas.com/articles/what-is-the-capital-of-france.html"
    },
    {
      "site_name": "France | History, Maps, Flag, Population, Cities, Capital, & Facts ...",
      "text_content": "Even though its imperialist stage was driven by the impulse to civilize that world accord",
      "url": "https://www.britannica.com/place/France"
    }
  ]
}
```

</details>

The summarize endpoint: `POST /query/summarize`
- incompatible with `--server` flag

<details> <summary> Example </summary>

Input:
```bash
curl -k "http://0.0.0.0:8080/query/complete" -d '{"search_config":{"api_key":"xxx"}, "backend":"tavily", "query": "Whats the capital of france"}'
```

Output:
```json
{
  "decision": true,
  "results": "1. Paris is the capital of France, located in the north-central part of the country. 2. It has a rich history and is known for its geography and climate. 3. The city's imperialist stage was driven by the impulse to civilize other parts of the world. 4. The historical district along the Seine in the city center has been classified as a UNESCO World Heritage Site.\</s>"
}
```

</details>

There are currently 3 supported backends, Tavily API, Bing API,
