# Provider integration feedback

## Nebius Token Factory

- Implemented the OpenAI-compatible `/models` lookup and environment-key test path.
- Live latency, model availability, chat quality, and streaming behavior still need to be recorded during a test with a Nebius key.

## Tavily

- The existing source-verification client now sends credentials using the documented `Authorization` header and does not include provider response bodies in its errors.
- General chat search/extract and live response behavior have not been tested in this update.
