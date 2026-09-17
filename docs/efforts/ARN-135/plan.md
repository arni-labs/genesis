# Scoped publication repair plan

1. Share ingestion's existing object-key function with app publication.
2. Resolve scoped commit keys through point reads, retaining repository-checked legacy keys and rejecting missing or mismatched objects.
3. Verify the observed production scoped-Id failure and both stored identity representations in unit tests. Exercise repaired publication against Genesis's pinned local runtime with a real Git push; verify an absent commit remains rejected. The pinned kernel stores raw SHA in fields.Id, so its original collection query succeeds and cannot reproduce production's scoped-Id rejection.
4. Review the finished delta, merge and deploy through the existing Genesis release route, then retry Katagami's publication and installation.

The result is a normal full-hash Katagami publication, with no duplicate object records, modified policies, or weakened commit validation.
