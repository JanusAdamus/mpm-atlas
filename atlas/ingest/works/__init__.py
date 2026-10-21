"""Works adapters (ronda 5, ADR-0005 §2–§4, §8–§9): zbMATH Open, OpenAlex, citations
(OpenCitations, Crossref), the Wikidata canon and the curated list of ancient witnesses with CDLI.

Every write goes through `Store.ingest_batch` (one batch = one transaction, resumable and
idempotent).  Metadata only; see `common.assert_metadata_only`."""
