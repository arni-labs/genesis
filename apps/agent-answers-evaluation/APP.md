# Agent Answers Evaluation

This bundle supplies versioned trial suites, trial metric definitions, and
validator runs for `agent-answers`. A campaign freezes a Genesis ref to this
bundle before candidate trials start. A Codex brain may publish a new evaluator
version for a future generation, but cannot change the evaluator active for a
running generation.

It intentionally models measurement capabilities rather than prescribing a
single fitness vector. The initial suite can use native/runtime facts,
deterministic validators, Datadog observations, or labeled agent judgments.
