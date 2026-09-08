# RFCs accepted and awaiting implementation

Empty by design when nothing is accepted — but this file must exist.

Git does not track empty directories, so without a keeper file this folder
vanishes from a fresh clone. `rfc-status-folder` reads `proposed/`, `accepted/`
and `done/` and **fails closed** when one cannot be read, so an absent folder
turns Gate 12 red for everyone who clones the repository while looking fine in
the working tree that created it.

This folder emptied for the first time at the `0.28.0` cut, when RFC-0.28-001
through `-005` all moved to `done/` together. The keeper was added during that
cut, after a clean-clone verification found `consistency-check` failing —
`rfc-status-folder`, `errata-tracking` and `doc-counts` all produced **no output
at all**, which is recorded as **E-038**.

`proposed/` carries the same stub for the same reason (`d5edf31`), and
`archive/` for the same reason again (RFC-0.25-002 R1). Three folders, three
separate discoveries of one property of git.
