# Implementation

1. Replace bare-SHA collection filtering for delta bases with repository-scoped point reads and checked legacy fallback. Preserve streamed response handling for large canonical fields.
2. Exercise the actual rebuilt WASM using locally generated Git packs, repository-prefixed rows, large blobs and deliberate identity defects. Show baseline failure and repaired success with the same input.
3. Review and deliver the module, then upload and read back the existing Monty object through bounded packs without moving release refs.

The existing older-runtime context overwrite and client/server body limits are reproduced constraints. Bounded packs avoid these limits; this change does not upgrade the kernel or widen upload permissions.
