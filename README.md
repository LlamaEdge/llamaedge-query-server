# LlamaEdge Query Server
<!-- @import "[TOC]" {cmd="toc" depthFrom=1 depthTo=6 orderedList=false} -->
<!-- code_chunk_output -->

- [LlamaEdge Query Server](#llamaedge-query-api-server)
  - [Introduction](#introduction)
  - [Quick Start](#quick-start)
    - [Endpoints](#endpoints)
      - [`POST /query/decide`](#post-querydecide)
      - [`POST /query/complete`](#post-querycomplete)
      - [`POST /query/summarize`](#post-querysummarize)
  - [CLI Options](#cli-options)
<!-- /code_chunk_output -->

## Introduction

The LlamaEdge Query Server allows a chatbot to determine whether a user query requires an internet search to answer. Additionally, it can also complete these searches as well as perform summarization on them (non `--server` mode).

## Quick Start

While the server itself doesn't make a distinction between what chat/instruct model is used (as long as it supports function calling), The best results have been observed on [Mistral Instruct V0.3](https://huggingface.co/second-state/Mistral-7B-Instruct-v0.3-GGUF). Larger models should generally offer more accurate decisions as well as better summaries. If you use another model, ensure it's supported by the `llama-core` backend. Check the --prompt-template option in the [cli options](#cli-options)

#### Build
```
cargo build --release --target wasm32-wasip1
```

#### Execute

```bash
wasmedge --dir .:.  --env LLAMA_LOG="info" \
		--nn-preload default:GGML:AUTO:Mistral-7B-Instruct-v0.3-Q5_K_M.gguf \
		./target/wasm32-wasip1/release/llamaedge-query-server.wasm \
		--ctx-size 4096 \
		--prompt-template mistral-tool \
		--model-name Mistral-7B-Instruct-v0 \
		--temp 1.0 \
		--log-all
```

Ensure that the Model (Mistal Instruct in this case) is present in the working directory.

## Endpoints

There are 3 endpoints: `decide`, `complete`, `summarize`

#### `POST /query/decide`

- Consults the LLM about whether the `query` passed in requires an internet search, and return `true` or `false` along with the produced query, if any.

<details> <summary> Example </summary>

Input
```bash
curl -k "http://0.0.0.0:8080/query/decide" -d '{"query": "Whats the capital of france"}'
```

Output:
```json
{
  "decision": true,
  "query": "What is the capital of France"
}
```

</details>

#### `POST /query/complete`

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

#### `POST /query/summarize`
- incompatible with `--server` flag. Descretion is advised when using on a server.

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

There are currently 2 supported search API backends, Tavily and Bing.

## CLI Options

Here are all the CLI options for the LlamaEdge Query Server.

```
Usage: llamaedge-query-server.wasm [OPTIONS] --prompt-template <PROMPT_TEMPLATE>

Options:
  -m, --model-name <MODEL_NAME>
          Sets names for chat model [default: default]
  -a, --model-alias <MODEL_ALIAS>
          Model aliases for chat model [default: default]
  -c, --ctx-size <CTX_SIZE>
          Sets context sizes for the chat model [default: 1024]
  -b, --batch-size <BATCH_SIZE>
          Sets batch sizes for the chat model [default: 512,512]
  -p, --prompt-template <PROMPT_TEMPLATE>
          Sets prompt templates for the chat model [possible values: llama-2-chat, llama-3-chat, llama-3-tool, mistral-instruct, mistral-tool, mistrallite, openchat, codellama-instruct, codellama-super-instruct, human-assistant, vicuna-1.0-chat, vicuna-1.1-chat, vicuna-llava, chatml, chatml-tool, internlm-2-tool, baichuan-2, wizard-coder, zephyr, stablelm-zephyr, intel-neural, deepseek-chat, deepseek-coder, deepseek-chat-2, solar-instruct, phi-2-chat, phi-2-instruct, phi-3-chat, phi-3-instruct, gemma-instruct, octopus, glm-4-chat, groq-llama3-tool, embedding, none]
  -r, --reverse-prompt <REVERSE_PROMPT>
          Halt generation at PROMPT, return control
  -n, --n-predict <N_PREDICT>
          Number of tokens to predict [default: 1024]
  -g, --n-gpu-layers <N_GPU_LAYERS>
          Number of layers to run on the GPU [default: 100]
      --no-mmap <NO_MMAP>
          Disable memory mapping for file access of chat models [possible values: true, false]
      --temp <TEMP>
          Temperature for sampling [default: 0.0]
      --top-p <TOP_P>
          An alternative to sampling with temperature, called nucleus sampling, where the model considers the results of the tokens with top_p probability mass. 1.0 = disabled [default: 1.0]
      --repeat-penalty <REPEAT_PENALTY>
          Penalize repeat sequence of tokens [default: 1.1]
      --presence-penalty <PRESENCE_PENALTY>
          Repeat alpha presence penalty. 0.0 = disabled [default: 0.0]
      --frequency-penalty <FREQUENCY_PENALTY>
          Repeat alpha frequency penalty. 0.0 = disabled [default: 0.0]
      --llava-mmproj <LLAVA_MMPROJ>
          Path to the multimodal projector file
      --socket-addr <SOCKET_ADDR>
          Socket address of LlamaEdge API Server instance [default: 0.0.0.0:8081]
      --log-prompts
          Deprecated. Print prompt strings to stdout
      --log-stat
          Deprecated. Print statistics to stdout
      --log-all
          Deprecated. Print all log information to stdout
      --max-search-results <MAX_SEARCH_RESULTS>
          Fallback: Maximum search results to be enforced in case a user query goes overboard [default: 5]
      --size-per-search-result <SIZE_PER_SEARCH_RESULT>
          Fallback: Size limit per result to be enforced in case a user query goes overboard [default: 400]
      --server
          Whether the server is running locally on a user's machine. enables local-search-server usage and summariztion
  -h, --help
          Print help
  -V, --version
          Print version
```
