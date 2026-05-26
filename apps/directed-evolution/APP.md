# Directed Evolution

`directed-evolution` is the reusable control app for evolving Temper-native
applications. It stores campaign direction, frozen selection designs,
generations, candidates, measurements, emergent capabilities, releases, and
human interventions as ordinary Temper entities.

The evolved application and the evaluator/trial app remain separate Genesis
bundles. A generation records both immutable refs so an observer can tell
whether a changed result came from an organism mutation or from a changed
judge.

## Safety Contract

- A selection design must be approved and frozen before a generation starts.
- Candidate mutations never change the frozen evaluator ref for that generation.
- Measurements carry their source and evidence locator rather than collapsing
  evidence into an unexplained score.
- An approved campaign may auto-release a selected candidate, but it must
  remain pausable and roll back to a recorded release ref.

The first consumer is the `agent-answers` demonstration app. Nothing in this
bundle prescribes that domain or a fixed fitness vector.
