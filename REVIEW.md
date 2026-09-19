# Reviewing genesis

Use the installed Stack review contract, or the bundled [Stack review contract](.stack/REVIEW.md) when Stack is not installed. Review the accepted outcome and changed behavior; independently exercise the feature when useful. Report concrete defects introduced or worsened by this change, with reproduction evidence and location. No mandatory panel, review markers, JSON record or unrelated cleanup.

Apply these repository checks only where the change touches them:

- Exercise changed git protocol behavior with real git: push/clone/fetch round trips, object hashes and `git fsck`. Check bytes against the Git contract.
- Check changed API response shapes with `gh` or `curl`, including errors.
- Test repository-scoped object/ref identity, authorization and credential handling; denied writes must leave state unchanged.
- For storage and pack changes, exercise partial writes, retries and recovery; check bounded resource use and acknowledged-write durability.

Report what you tested, the revision, findings and material limits in plain language.
