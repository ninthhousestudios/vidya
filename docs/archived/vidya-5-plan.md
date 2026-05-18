# vidya/5 implementation plan — remaining slices

Task: Full fact layer — claims, relations, epistemology

## Completed (this session)

### Slice 1-2: Source slug dedup + seed loading
- Migration 0002: added `slug` column to sources table with unique index
- Updated `db.rs`: `SourceRow` includes slug, added `upsert_source`, `get_source_by_slug`
- Updated `vidya_load`: sources use ON CONFLICT (slug) for idempotent loading
- Updated `vidya_claim`: inline source creation uses slug derived from reference
- Integration tests: `load_vyakarana_seed`, `load_jyotish_seed` — both pass with exact count assertions and idempotent reload verification

### Slice 3: Param schema validation in vidya_load
- Added `jsonschema` crate dependency
- vidya_load validates all claim params against template's `param_schema` before any inserts
- Returns actionable error: `claims[N].params: must match template 'X' param_schema at /field: <error detail>`
- Test: `load_rejects_invalid_claim_params` — passes

## Remaining (next session)

### Slice 4: Param schema validation in vidya_claim

**Test to write:**
```rust
#[tokio::test]
async fn claim_create_rejects_invalid_params() {
    // Setup domain with claim_template that has param_schema with required fields and types
    // Call vidya_claim create with params that violate the schema
    // Assert error mentions the specific field that failed validation
}
```

**Implementation:**
- In `src/tools/claim.rs`, in the "create" action, after fetching the template:
  - Build a `jsonschema::Validator` from `template.param_schema`
  - Validate `params` against it
  - Return `VidyaError::InvalidArgument` with actionable message on failure
- Same pattern as vidya_load validation — reuse the error formatting

### Slice 5: Claim status transitions

**Test to write:**
```rust
#[tokio::test]
async fn claim_status_transitions() {
    // Create claim with status "proposed"
    // Update to "active" — succeeds
    // Update to "historical" — succeeds
    // Create another claim, try "proposed" → "historical" directly — should this be allowed?
    // Try "active" → "proposed" — fails (invalid transition)
    // Try update on nonexistent ID — fails
}
```

**Implementation:**
- Add "update" action to `ClaimArgs` (needs `id` field, already exists)
- Add `db::update_claim_status(pool, id, new_status) -> Result<ClaimRow>`
- Enforce valid transitions:
  - `proposed → active` (approve)
  - `proposed → historical` (reject/skip — allowed per PRD "immutable once active")
  - `active → historical` (supersede)
  - All others rejected
- Return the updated claim row

### Slice 6: vidya_relation tool

**Test to write:**
```rust
#[tokio::test]
async fn relation_create_and_list() {
    // Load a domain with entity_kinds, relation_kinds, entities
    // Create a relation via vidya_relation tool (action: "create")
    // List relations for an entity — verify it appears
    // Get relation by ID — verify details
}
```

**Implementation:**
- New file: `src/tools/relation.rs`
- `RelationArgs`: action (create/get/list), domain, kind, src_entity (name), dst_entity (name), optional src_domain/dst_domain for cross-domain
- `RelationOutput`: action, relation, relations
- Actions:
  - `create`: lookup domain, relation_kind, src entity (by name+domain), dst entity (by name+domain), insert
  - `get`: by UUID
  - `list`: by entity name (returns all relations involving that entity)
- Register in `src/tools/mod.rs` and `src/mcp.rs`
- Update `src/mcp.rs` `ServerInfo.instructions` string

### Slice 7: Cross-domain relations

**Test to write:**
```rust
#[tokio::test]
async fn cross_domain_relation() {
    // Load vyakarana domain (has entity "a" of kind "varna")
    // Load a separate small domain with an entity
    // Create relation where src is in vyakarana and dst is in the other domain
    // Verify the relation is created and both entities are from different domains
}
```

**Implementation:**
- In vidya_relation create: `src_domain` and `dst_domain` params (optional, default to the relation's domain)
- Entity lookup resolves across domains: `db::get_entity_by_name(pool, domain_id, name)` — for cross-domain, use the specified domain_id
- The relation's `domain_id` is the domain of the relation_kind (the domain that defines the relationship vocabulary)

### Slice 8: Jyotish query-only verification

**Test to write:**
```rust
#[tokio::test]
async fn jyotish_query_only() {
    // Load jyotish.json
    // Query entities by kind "graha" — get 9 planets
    // Query claims by template "dignity" — get all dignity claims
    // Query entity "Sūrya" — get claims and relations
    // Verify vidya_derive for jyotish returns "no engine strategy" error (not crash)
}
```

**Implementation:**
- Should already work after slices 1-2 pass. This test just confirms query-only domain works.
- May need to verify the derive error message is clean for domains without engine strategies.

### Slice 9: Full integration test

**Test to write:**
```rust
#[tokio::test]
async fn full_integration_both_seeds() {
    // Load vyakarana.json then jyotish.json
    // Verify entity counts per domain
    // Verify claim counts per domain
    // Verify relations exist in jyotish
    // Verify assertions exist (check one specific claim's assertion has correct tradition + source)
    // Verify traditions are hierarchical (kāśikā has parent pāṇini)
    // Verify sources have reliability scores
    // Query across: vidya_query for Sūrya in jyotish → entities + claims + provenance
}
```

**Implementation:**
- Mostly a test — exercises the full system. May reveal edge cases in query.rs when dealing with provenance across loaded data.

## Architecture notes for implementor

### Files to modify:
- `src/tools/claim.rs` — add validation + update action
- `src/tools/relation.rs` — new file
- `src/tools/mod.rs` — add relation module
- `src/mcp.rs` — add vidya_relation tool endpoint
- `src/db.rs` — add `update_claim_status`
- `tests/integration.rs` — add remaining tests

### Key patterns:
- Validation uses `jsonschema::Validator::new(&schema)` then `.validate(&instance)` — returns `Result<(), ValidationError>`
- Error formatting: use `error.instance_path().to_string()` for field path
- Source dedup: `upsert_source` with ON CONFLICT (slug)
- Cross-domain entity lookup: pass explicit domain_id to `get_entity_by_name`

### Status transition matrix:
| From | To | Allowed |
|------|-----|---------|
| proposed | active | yes |
| proposed | historical | yes |
| active | historical | yes |
| active | proposed | NO |
| historical | * | NO |
