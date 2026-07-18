# clearhead-graphd

Standalone graph/query process for ClearHead. The first implementation is a
one-shot binary; the process boundary is intentional even though graphd still
uses `clearhead-core`'s graph module internally.

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
  }
}
```

`config` and both of its fields may be omitted. Unknown request fields and
unsupported versions are rejected so contract drift is visible.

On success, stdout contains only a JSON array of string-valued binding maps:

```json
[{"s":"urn:uuid:..."}]
```

An empty result is `[]`. Human formatting remains the client's concern, so the
existing CLI JSON/table output shapes do not become graphd protocol details.
Warnings and errors go to stderr. A failed request exits non-zero and stdout
must not be consumed.

The CLI locates the executable as `clearhead-graphd` on `PATH`. Set
`CLEARHEAD_GRAPHD` to an explicit executable path when packaging or testing.

## Current scope

- load the primary workspace and resolved additional workspaces
- apply tag hierarchy configuration while materializing the graph
- execute raw SPARQL supplied by the client
- return raw rows through the versioned contract

Named query resolution, index framing, and chain target resolution remain in
the CLI for the next transition slices.
