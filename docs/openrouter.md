# OpenRouter provider

`OpenRouterClient` implements the existing domain `AiClient` for roleplay and summaries. The existing `LlmRouter` retry behavior applies. No database migration is needed.

## Configuration

- `OPENROUTER_API_KEY`: secret; optional while another provider is active.
- `OPENROUTER_MODEL`: defaults to `qwen/qwen3.8-27b`.
- Select `openrouter` using the existing `app_config` key `active_llm_provider`. This DB value takes priority over `DEFAULT_LLM_PROVIDER`, including when it is still set to `together`.
- `DEFAULT_LLM_PROVIDER=openrouter` takes effect when the DB key is absent (or the config lookup fails).

Changing the model env requires restarting the backend. Provider selection uses the existing config cache. This integration does not switch production automatically. Missing key/model fails before making any request if OpenRouter is selected.

## Compatibility

Roleplay requires a model/endpoint supporting `tools` and a named `tool_choice`. The request uses `provider.require_parameters=true` to avoid endpoints silently ignoring those parameters. Models that only produce plain text (including some creative/uncensored models) are not supported for roleplay by this adapter.

Calls use non-streaming JSON responses with a 120-second request timeout, matching the existing `AiClient` complete-response contract; this does not add browser token streaming. Truncated replies, missing/invalid scene state, and API error envelopes are rejected. Summary requests omit tools. Upstream error bodies are not retained in errors because they can contain sensitive request content.

## Verification

Local tests use an HTTP stub, never production credentials. A separate ignored smoke test exercises the actual client for both operations using synthetic Thai dialogue:

```sh
cargo test --test openrouter_live -- --ignored --nocapture
```

Run from `backend/` with the key in the environment or `.env`. This test incurs OpenRouter API usage and prints synthetic answers for manual Thai/persona review. A passing test validates the response contract, not overall roleplay quality. It does not alter the active provider or create production stories.
