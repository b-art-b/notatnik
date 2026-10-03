# Phase 7: AI features (optional, off by default)

**Goal:** local-first AI that makes your notes easier to find and use: semantic and hybrid search,
related notes, and "chat with my notes" with verifiable citations. No note content leaves your
devices unless you explicitly configure a remote provider.

**Requirements covered:** AI-1–4, AI-6, SR-3, SR-4 (if not done), AI-5 (stretch).
**Depends on:** phase 1 index; phase 2 UI; phase 6 recommended (AI can be exposed to plugins with the `ai` permission).

## Exit criteria

1. Every task checked, with every **Verify** item passing; `scripts/check.sh --full` passes, including the retrieval evaluation (P7.8).
2. Retrieval quality on the evaluation set: recall@5 ≥ 0.8 for hybrid search, and better than full-text alone.
3. The privacy tests (P7.9) pass.
4. With AI disabled, the app makes **zero** AI-related network calls (verified).

## New crate

`crates/ai` (`notatnik-ai`): provider trait, Ollama client, chunking, embedding queue, retrieval, prompt building.

## Key design decisions for this phase

| Topic | Decision |
|---|---|
| Provider | `Provider` trait with `embed(texts) -> vectors` and `chat(messages, stream) -> tokens`. First implementation: **Ollama** HTTP API (local, or on a home server; the same config works from Android later). Remote providers (e.g. Anthropic, OpenAI-compatible endpoints) are optional implementations, configured explicitly. |
| Embedding model | Must be **multilingual** (Polish and English notes). Candidates: `bge-m3`, `multilingual-e5` variants, `nomic-embed-text` v2 (multilingual); pick by the P7.8 evaluation, record the decision. |
| Vector storage | `sqlite-vec` inside the existing index DB. Note: registering the extension needs `unsafe` FFI; allow `unsafe_code` *only* in the small module that does it, with a comment justifying it (the workspace forbids it elsewhere). |
| Chunking | One chunk = one block plus its context: page title + parent breadcrumb + the block text (children not included). Very short blocks (< 5 words) are merged with their parent context or skipped. |
| Incrementality | Each chunk stores `content_hash` and `model_id`. Re-embed only when the hash changes; changing the model triggers a background full re-embed. |
| Retrieval | Hybrid: FTS5 BM25 top-50 + vector KNN top-50 → reciprocal rank fusion → top-k. Namespace exclusions are applied **before** anything is sent to a provider. |
| Citations | The chat prompt lists retrieved blocks with their IDs; the model is told to cite `((id))`. The UI validates every cited ID against the retrieved set and flags invented ones. |

---

## 7A: Provider and embeddings

### P7.1 Ollama setup (you)
- [ ] Install Ollama (`curl -fsSL https://ollama.com/install.sh | sh`, or run it on a home server); pull one embedding model candidate and one chat model (e.g. a small instruct model that runs on your hardware)

**Verify**
- [ ] `curl http://localhost:11434/api/tags` lists the pulled models
- [ ] `ollama run <chat-model> "hello"` answers

### P7.2 Provider trait and Ollama client (AI-1)
- [ ] `embed`, `chat` (streaming), health check, model listing; timeouts and clear errors when Ollama isn't running
- [ ] Settings → AI: enable toggle (off by default), endpoint, embedding model, chat model, "test" button

**Verify**
- [ ] Unit tests against a mock HTTP server (`wiremock`): request shapes, streaming parsing, error mapping
- [ ] With Ollama stopped → the settings test shows "can't reach Ollama at …", and the app keeps working

### P7.3 Embedding pipeline (AI-2)
- [ ] Background queue fed by `graph-changed`; batches; pause while the user is typing; a progress indicator ("embedding 1,234 / 20,000")
- [ ] `vec_blocks` table with `content_hash`, `model_id`

**Verify**
- [ ] Editing one block → exactly one chunk re-embedded (counted in the mock provider)
- [ ] Changing the embedding model → full re-embed, with old vectors removed when done
- [ ] Synthetic graph: initial embedding completes (record the time and throughput in benchmarks.md); the UI stays responsive (typing latency within NF-4) while it runs
- [ ] Killing the app mid-embedding → on restart, it resumes where it left off and doesn't start from zero

## 7B: Retrieval

### P7.4 Semantic and hybrid search (SR-3)
- [ ] Search dialog gains a "semantic" toggle (hybrid by default when AI is enabled); results show why they matched (keyword vs meaning)

**Verify**
- [ ] On the AI evaluation set: the query "how do I reset the router" finds a block saying "restart the modem by holding the button" (no shared keywords)
- [ ] A Polish query finds an English block with the same meaning, and vice versa (if the model is multilingual)
- [ ] Hybrid search latency < 300 ms on the synthetic graph (excluding the first model load)

### P7.5 Related notes panel (AI-3)
- [ ] For the current page: the top N semantically similar pages (by averaged chunk vectors), excluding pages already linked

**Verify**
- [ ] On the evaluation graph, for 5 labelled pages, at least 2 of the expected related pages appear in the top 5

## 7C: Chat with notes

### P7.6 Chat UI and RAG (AI-4)
- [ ] Chat panel (sidebar): question → hybrid retrieval → prompt with the retrieved blocks → streamed answer with `((id))` citations rendered as clickable refs
- [ ] Scope selector: whole graph, current namespace, current page
- [ ] "Save answer to journal" button (inserts the answer as a block with its citations)

**Verify**
- [ ] On the evaluation set's chat questions: each answer cites at least one expected source block (automated check), and citations outside the retrieved set are flagged in the UI
- [ ] A question with no relevant notes → the answer says it couldn't find anything (the prompt instructs this; verified on 5 "unanswerable" questions)
- [ ] Streaming can be cancelled; cancelling stops the HTTP request

### P7.7 Block AI actions (AI-5, stretch)
- [ ] Right-click a block: summarise children, rewrite, extract tasks (creates TODO blocks), suggest tags/links. The result is shown as a preview → accept = normal `BlockOp`s (one undo step)

**Verify**
- [ ] "Extract tasks" on a meeting-notes block produces TODO children only after accepting; Ctrl+Z removes them all

## 7D: Quality and privacy

### P7.8 Retrieval evaluation
- [ ] `fixtures/graphs/ai-eval/` (≈ 200 blocks, Polish + English) with `questions.json`: query → expected source block IDs (≥ 40 queries: keyword, paraphrase, cross-language, unanswerable)
- [ ] `notatnik dev eval-retrieval` prints recall@5, recall@10 and MRR for full-text, vector and hybrid

**Verify**
- [ ] Hybrid recall@5 ≥ 0.8, and hybrid ≥ the better of full-text and vector alone
- [ ] Results for each candidate embedding model recorded in `docs/benchmarks.md`; the chosen model is documented as a decision

### P7.9 Privacy and exclusions (AI-6)
- [ ] Config: `[ai] exclude = ["private", "health"]` (namespaces; each covers the page and everything below it) plus a page-level `ai: false` frontmatter key
- [ ] Excluded content is never embedded, never retrieved and never sent; a remote provider requires an explicit confirmation dialog naming the endpoint

**Verify**
- [ ] Test with a recording mock provider: index + chat over a graph with excluded namespaces → no request body contains any text from excluded pages (string search over all recorded requests)
- [ ] With AI disabled: run the app for 10 minutes with editing and searching → no connections to the AI endpoint (mock server records zero requests; optionally verify with `ss -tp` or a firewall log)
- [ ] Switching from local to a remote provider shows the confirmation dialog; cancelling keeps the local provider

---

## Manual acceptance checklist

- [ ] Use semantic search for a week alongside normal search; note cases where it helped or misled
- [ ] Ask 10 real questions about your own notes; check every citation by clicking it
- [ ] Mark one namespace as excluded and confirm (via the mock or Ollama logs) that it never appears in requests

Record: date / commit / notes:
