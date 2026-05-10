-- vidya three-layer knowledge schema
-- Layer 1: Ontology (grammar of a domain)
-- Layer 2: Facts (instances)
-- Layer 3: Epistemology (provenance + pramana)

-- == Ontology layer ==

create table domains (
    id    uuid primary key default gen_random_uuid(),
    slug  text not null unique,
    title text not null
);

create table entity_kinds (
    id        uuid primary key default gen_random_uuid(),
    domain_id uuid not null references domains(id),
    slug      text not null,
    schema    jsonb,
    unique(domain_id, slug)
);

create table relation_kinds (
    id          uuid primary key default gen_random_uuid(),
    domain_id   uuid not null references domains(id),
    slug        text not null,
    src_kind_id uuid references entity_kinds(id),
    dst_kind_id uuid references entity_kinds(id),
    schema      jsonb,
    unique(domain_id, slug)
);

create table claim_templates (
    id           uuid primary key default gen_random_uuid(),
    domain_id    uuid not null references domains(id),
    slug         text not null,
    param_schema jsonb not null default '{}',
    unique(domain_id, slug)
);

-- == Fact layer ==

create table entities (
    id        uuid primary key default gen_random_uuid(),
    domain_id uuid not null references domains(id),
    kind_id   uuid not null references entity_kinds(id),
    name      text not null,
    attrs     jsonb not null default '{}',
    unique(domain_id, kind_id, name)
);

create table claims (
    id          uuid primary key default gen_random_uuid(),
    domain_id   uuid not null references domains(id),
    template_id uuid not null references claim_templates(id),
    params      jsonb not null default '{}',
    status      text not null default 'active'
                check (status in ('proposed', 'active', 'historical')),
    statement   text not null,
    created_at  timestamptz not null default now()
);

create table relations (
    id            uuid primary key default gen_random_uuid(),
    domain_id     uuid not null references domains(id),
    kind_id       uuid not null references relation_kinds(id),
    src_entity_id uuid not null references entities(id),
    dst_entity_id uuid not null references entities(id),
    attrs         jsonb not null default '{}',
    unique(domain_id, kind_id, src_entity_id, dst_entity_id)
);

-- == Epistemology layer ==

create table traditions (
    id        uuid primary key default gen_random_uuid(),
    domain_id uuid not null references domains(id),
    name      text not null,
    parent_id uuid references traditions(id),
    unique(domain_id, name)
);

create table sources (
    id          uuid primary key default gen_random_uuid(),
    kind        text not null check (kind in ('text', 'practitioner', 'derivation', 'oral')),
    reference   text not null,
    reliability real check (reliability >= 0.0 and reliability <= 1.0)
);

create table assertions (
    id            uuid primary key default gen_random_uuid(),
    claim_id      uuid not null references claims(id),
    tradition_id  uuid not null references traditions(id),
    source_id     uuid not null references sources(id),
    pramana       text not null
                  check (pramana in (
                      'pratyaksha', 'anumana', 'shabda',
                      'upamana', 'arthapatti', 'anupalabdhi'
                  )),
    confidence    real not null default 1.0
                  check (confidence >= 0.0 and confidence <= 1.0),
    asserted_at   timestamptz not null default now()
);

create table derivations (
    id                  uuid primary key default gen_random_uuid(),
    conclusion_claim_id uuid not null references claims(id),
    premise_claim_id    uuid not null references claims(id),
    step_order          int not null,
    created_at          timestamptz not null default now()
);

-- == Indexes ==

create index idx_entity_kinds_domain on entity_kinds(domain_id);
create index idx_relation_kinds_domain on relation_kinds(domain_id);
create index idx_claim_templates_domain on claim_templates(domain_id);
create index idx_entities_domain_kind on entities(domain_id, kind_id);
create index idx_entities_name on entities(name);
create index idx_claims_domain_template on claims(domain_id, template_id);
create index idx_claims_status on claims(status);
create index idx_relations_src on relations(src_entity_id);
create index idx_relations_dst on relations(dst_entity_id);
create index idx_assertions_claim on assertions(claim_id);
create index idx_assertions_tradition on assertions(tradition_id);
create index idx_derivations_conclusion on derivations(conclusion_claim_id);
create index idx_derivations_premise on derivations(premise_claim_id);
create index idx_traditions_domain on traditions(domain_id);

-- idempotency for bulk loading
create unique index idx_claims_dedup on claims(domain_id, template_id, md5(params::text));
