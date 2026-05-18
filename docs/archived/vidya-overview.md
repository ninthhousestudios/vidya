# Vidya: Codebase & Architecture Overview

This document provides a comprehensive overview of the `vidya` project, detailing what it is, how it works, and how it is implemented under the hood.

## What is Vidya?

Vidya is a structured knowledge graph designed for domain-specific reasoning with strict provenance tracking. It is a subsystem of the `manas` infrastructure, intended to give LLM agents access to cited, tradition-aware facts rather than relying on unstructured RAG (Retrieval-Augmented Generation) or pre-training weights.

Vidya solves the inherent limitations of RAG in complex domains (such as astrology or Sanskrit grammar) where:
- **Traditions disagree:** It scopes claims to specific traditions (e.g., Vedic vs. Western astrology) to prevent LLMs from inappropriately blending contradictory facts.
- **Rules are structural:** It models dense relational graphs (e.g., aspects, derivations, dignities) explicitly, rather than flattening them into text.
- **Provenance is required:** Every claim must be tied to a source (a text chunk, a practitioner's assertion, etc.).

## How it Works (The Core Concepts)

Vidya separates knowledge into a three-layer schema backed by a PostgreSQL database:

1. **Ontology Layer:** Defines the grammar of the domain. It consists of `domains`, `entity_kinds`, `relation_kinds`, and `claim_templates`. Claim templates define the JSON schema required for claims in that domain.
2. **Fact Layer:** Contains the actual knowledge—`entities` (e.g., Saturn), `claims` (e.g., "Saturn is exalted in Libra"), and `relations` (typed edges between entities and claims). Claims are immutable; updates are managed by creating new claims that supersede old ones.
3. **Epistemology Layer:** Tracks the origin and reliability of knowledge via `traditions`, `sources`, `assertions` (who claims this and with what confidence), and `derivations` (logical steps showing how a claim was inferred from others).

To reason over this data, Vidya employs an **Engine Strategy**:
Domains that require active reasoning (like `vyākaraṇa` / Sanskrit grammar) implement the `EngineStrategy` Rust trait. This trait provides two primary operations:
- **Derivation (`derive` - forward logic):** Given an input and operation, applies matching rules from the knowledge graph to produce a deterministic result with a step-by-step trace.
- **Analysis (`analyze` - reverse logic):** Given a surface form, reverse-engineers and enumerates valid ranked decompositions, citing the rules that could produce it.

## How it Does It (Implementation & Architecture)

Vidya is built in Rust as a binary crate (`src/main.rs`) that runs as a server providing a Model Context Protocol (MCP) tool surface.

### 1. Server and MCP Layer (`src/mcp.rs`, `src/tools/`)
Vidya uses `rmcp` (Rust MCP) to expose its capabilities to agents. It supports both standard I/O (for local CLI integration like Claude Code) and streamable HTTP with Bearer token authentication.

The MCP tools exposed by `VidyaServer` include:
- `vidya_domain`, `vidya_entity`, `vidya_claim`, `vidya_relation`: CRUD operations mapped to the structured Postgres schema.
- `vidya_query`: Exposes structural queries without requiring agents to understand SQL.
- `vidya_load`: Accepts bulk seed data (like the JSON files found in the `seeds/` directory), validates it against the domain's ontology (using JSON Schema), and idempotently inserts it.
- `vidya_derive` & `vidya_analyze`: The entry points for forward and reverse domain reasoning.

The implementations for these tools live in `src/tools/` (e.g., `domain.rs`, `claim.rs`, `query.rs`, `derive.rs`, `analyze.rs`, `load.rs`).

### 2. Reasoning Engine (`src/engine/`)
The `Engine` (defined in `src/engine/mod.rs`) is responsible for executing the `EngineStrategy`. When an MCP `vidya_derive` or `vidya_analyze` call comes in, the Engine dispatches it to the specific domain logic. 
Currently, the codebase contains implementations for Sanskrit grammar reasoning:
- `sandhi.rs` (phonetic combinations)
- `declension.rs` (noun inflections)
- `phoneme.rs` (phonetic base logic)

These modules define how to load structural claims from the database and execute them as logical rules, resolving rule conflicts based on domain-specific metadata (like Sūtra ordering or rule types like *utsarga* vs. *apavāda*).

### 3. Database Layer (`src/db.rs`, `migrations/`)
The persistence layer heavily relies on `sqlx` and PostgreSQL.
- Migrations (`0001_schema.sql`, `0002_source_slug.sql`) define the rigorous relational structure supporting the 3-layer schema.
- `src/db.rs` manages the connection pool and provides the foundational data access queries used by the MCP tools and the reasoning engine.

#### The 3-Layer Schema Breakdown

The database is built on UUID primary keys and leverages `jsonb` for flexible, schema-validated parameters.

**1. Ontology Layer (Domain Grammar)**
Defines the rules and structure for a given domain.
*   **`domains`**: The root aggregate (`id`, `slug`, `title`).
*   **`entity_kinds`**: Defines types of things that can exist in a domain (e.g., "planet", "phoneme"). Optionally contains a JSON Schema for the entity's attributes.
*   **`relation_kinds`**: Defines typed directed edges between entities. It can enforce that the source and destination entities are of specific kinds.
*   **`claim_templates`**: The blueprint for structured facts. Crucially, it has a `param_schema` (JSON Schema) that validates the payload of any claim instantiated from this template.

**2. Fact Layer (Instances)**
Holds the actual data instances defined by the ontology.
*   **`entities`**: The nodes in the knowledge graph. 
*   **`claims`**: Structured statements of fact. Claims are immutable. They have a `status` which acts as a lifecycle (`proposed`, `active`, `historical`). The exact data of the claim lives in the `params` JSONB field (validated against the `claim_templates.param_schema`). A unique MD5 hash of the params prevents duplicate claims.
*   **`relations`**: Instantiated edges between entities.

**3. Epistemology Layer (Provenance & Belief)**
Answers *why* the system believes a claim and *where* it came from.
*   **`traditions`**: Hierarchical scopes for knowledge (e.g., "Vedic Astrology" vs "Western Astrology").
*   **`sources`**: Citations pointing to where the knowledge originated. A source has a `kind` (`text`, `practitioner`, `derivation`, `oral`), a `reference`, a `slug` for deduplication, and a `reliability` score.
*   **`assertions`**: The mapping that ties a `claim` to a `tradition` and a `source`. It includes a `confidence` score and a `pramana` (the philosophical "means of knowledge", constrained to types like `pratyaksha` (perception), `anumana` (inference), etc.). 
*   **`derivations`**: The reasoning trace. If a claim was generated by the reasoning engine, this table stores the steps, linking a `conclusion_claim_id` back to its `premise_claim_id`(s) with an integer `step_order`.

### Summary of Execution Flow
1. A domain expert or LLM curates knowledge into JSON seed files (`seeds/`).
2. An agent uses `vidya_load` to ingest this into the PostgreSQL database. The data is validated against the defined `claim_templates`.
3. An LLM agent queries the system using `vidya_query` to retrieve facts scoping to a specific tradition.
4. If generative reasoning is required (e.g., conjugating a word), the agent uses `vidya_derive`. The `VidyaServer` passes this to the `Engine`, which executes the Rust trait implementation (`sandhi` or `declension`) against the stored facts, returning a fully traceable result to the agent.