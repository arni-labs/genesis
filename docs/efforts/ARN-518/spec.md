# Thin-pack bases use durable object identities

Given a thin Git pack referencing a base object previously ingested into the same repository, IngestPack must resolve that object by the same repository-scoped identity ingestion writes. It must verify repository membership, canonical object kind and SHA before using its bytes. Legacy bare-SHA rows remain readable only when they match the requested repository and hash. Missing bases and mismatched objects fail without committing sub-writes.

The runtime contract is unchanged: object staging never moves a ref unless RefUpdates explicitly requests it. A bounded sequence of standard Git delta packs must reconstruct the exact target blob, as verified by Git's parser and byte comparison.

State model: Absent -> Durable(base); Durable(base) + ValidDelta -> Durable(base,target); any failed lookup, identity check or delta -> unchanged durable state. Tests express these cases against the actual WASM module.
