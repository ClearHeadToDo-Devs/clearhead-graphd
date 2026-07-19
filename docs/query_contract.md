# Query Output Contract

**Status:** Stable

## Overview

This document governs the **semantics of query and view output** — the result of
running a saved or ad-hoc query against a workspace graph. It is graphd's public
interface for query results: the invariants the emitted JSON-LD obeys, regardless
of which query produced it.

It sits alongside two neighbours:

- [`README.md`](../README.md) — the **wire** contract: how a client invokes a
  query and what the request/response envelope looks like.
- [`jsonld_export_contract.md`](./jsonld_export_contract.md) — the **field**
  contract: the exact node types, required fields, sort order, and SHACL shapes
  of the emitted document.

This document is the layer above both: the principles those concrete artifacts
realize. It does **not** define the vocabulary or the `@context` — those are
owned by the `ontology` artifacts; this describes how query output *applies*
them. How a client *renders* the output is not covered here — that is the CLI's
presentation contract, and by design a client concern.

## Core principle: one semantic payload

A query emits **one** JSON-LD document. graphd never forks its bytes per
consumer: meaning travels with the data, and consumers **opt out of depth**,
never into it. Simple readers take the `@graph` array and ignore the rest;
semantic consumers honor the `@context`.

Lighter presentations — a terminal table, one-record-per-line NDJSON, a nested
tree — are not graphd forking its output. They are **downstream projections** a
presentation client derives from this one payload. The reshaping lives in the
client; graphd stays single-output. The seam carries meaning; the client spends
it.

## Identity is semantic `@id`, exported as client-facing `id`

Every node carries the **canonical workspace identity** the graph knows it by.
Semantically, in JSON-LD, that identity is `@id`. Producers **SHOULD compact**
`@id` to plain `id` via the payload's `@context`, so a direct JSON reader does
not need to special-case an `@` key.

So the rule is:

- **`@id`** — the JSON-LD meaning,
- **`id`** — the compacted surface simple readers use.

That identity is:

- the join key between a displayed entry and the thing it refers to,
- the address a mutation verb targets,
- **never** a throwaway or presentation-local identifier.

A consumer holding a node's canonical identity can act on exactly that node.

## Query form follows data shape

Serialization is JSON-LD regardless; the SPARQL *form* follows the natural shape
of the data:

- **`SELECT`** — for ordered lists and trees. Bindings are framed into `@graph`
  nodes; one projected variable binds the node IRI (exported as `id`).
- **`CONSTRUCT`** — for genuine networks (nodes with many edges). Emits an RDF
  graph serialized directly as JSON-LD.

Choosing JSON-LD does **not** force `CONSTRUCT`. Order-bearing views stay
`SELECT` — a `CONSTRUCT` result is a set of triples and cannot carry row order.

## Shape is edge count

List, tree, and network are not distinct formats. They are the same node-set
differing only in how many canonical-identity-valued edge properties each node
carries:

- **list** — no edge properties,
- **tree** — one hierarchical edge (e.g. `parent`),
- **network** — several edges.

The serialization is identical. This shape is the **hinge to presentation**: it
is metadata a client reads — from the query's declared shape in graphd's
registry — to choose how it renders and re-serializes the one payload. graphd
carries the shape; it does not act on it.

## Ordering

A graph is unordered; the agenda and similar views are ordered. Order is carried
two ways, belt and suspenders:

1. `@graph` is a JSON array; a `SELECT`'s `ORDER BY` is preserved in array
   position, which direct JSON readers consume as-is.
2. The sort keys are **also** emitted as node properties, so a consumer that
   round-trips through an RDF store (where `@graph` is a set and array order is
   not guaranteed) can recover the ordering.

Producers **MUST** emit sort keys as properties for any ordered view. Consumers
**MAY** rely on array order for direct reads.

## Contract validation

Each response type declares the node properties its consumer requires — identity,
locator, display fields, sort keys. graphd validates output against the declared
contract and errors clearly when properties are missing. It does **not** compose,
inject, or repair: a query either satisfies its contract or fails loudly.

## The seam

graphd is a **stateless producer**. It runs SPARQL and emits the JSON-LD
document; it holds no session and no mutable result set.

Mutation is a separate set of **verbs addressed by canonical identity** (complete,
update, cancel, …) — each a targeted, single-node, single-file write.

"Living" views are not an engine feature. They are a client composing
**read → act → re-read**: run the query, fire a verb by canonical identity,
re-run the query. A human editor, an agent, and a GUI drive the *identical* loop
over the *identical* primitives. This is the test of the seam: if any consumer
needs an endpoint the others don't get, the seam has leaked.

graphd **MUST NOT** know about client widgets. Output named after one client's
widget (e.g. a "quickfix list") is a leak; the contract is client-neutral —
identified nodes plus locator, display, and sort-key properties — and each client
maps it to its own substrate.

### Errors as data

Verb failures (already-complete, not-found, conflict) are returned as structured,
branchable results, not prose on stderr, so an automated loop can respond rather
than guess.

## Aggregates (out of scope)

Pure aggregate/analytic queries (COUNT, AVG, GROUP BY, completion velocity) return
computed values with no identity — nothing to navigate to, nothing to address.
They are the deliberate exception to the one-semantic-payload rule: they use
`SELECT` and return scalar/tabular results, not a JSON-LD node set.

> **Open:** whether aggregate results ever warrant wrapping as observation nodes
> (so they too carry `@id` and rejoin the graph) is deferred to the
> analytics/review work, not settled here.
