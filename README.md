# clearhead-graphd

Standalone graph/query process for ClearHead. The first implementation is a
one-shot binary; the process boundary is intentional even though graphd still
uses `clearhead-core`'s graph module internally.

The boundary rule is simple: **the CLI speaks plain JSON; graphd owns JSON-LD**.

## Query contract (version 1)

Invocation:

```sh
clearhead-graphd --workspace <workspace-root> query
```

The client writes one JSON request to stdin:

```json
{
  "version": 1,
  "sparql": "SELECT ?s WHERE { ?s ?p ?o }",
  "config": {
    "tag_hierarchies": {},
    "additional_workspaces": []
  },
  "output": "rows"
}
```

`config`, its fields, and `output` may be omitted. `output` defaults to `rows`.
Unknown request fields and unsupported versions are rejected so contract drift
is visible.

### Plain row output

With `"output":"rows"`, stdout contains a JSON array of string-valued binding
maps:

```json
[{"s":"urn:uuid:..."}]
```

This is used by raw and aggregate queries. Human table formatting remains the
CLI's concern.

### Index JSON-LD output

With `"output":"index_jsonld"`, graphd validates the query projection against
the index contract and emits the canonical JSON-LD document:

```json
{"@context": {"id":"@id"}, "@graph": [{"id":"urn:uuid:..."}]}
```

The CLI may print that document directly or treat its `@graph` array as plain
JSON when rendering a table; it does not construct the JSON-LD context itself.

## Domain JSON to JSON-LD export

Invocation:

```sh
clearhead-graphd export-jsonld
```

stdin is a JSON-encoded `DomainModel`; stdout is canonical JSON-LD. This keeps
filtering and ordinary JSON/domain handling available to CLI commands while
making graphd the sole process that exports graph-shaped data.

## Process behavior

Warnings and errors go to stderr. A failed request exits non-zero and stdout
must not be consumed. The CLI locates the executable as `clearhead-graphd` on
`PATH`; set `CLEARHEAD_GRAPHD` to an explicit executable path when packaging or
testing.

## Current scope

- load the primary workspace and resolved additional workspaces
- apply tag hierarchy configuration while materializing the graph
- execute raw, named, index, and chain SPARQL supplied by clients
- return plain binding rows or graphd-framed index JSON-LD
- export JSON domain models as canonical JSON-LD
