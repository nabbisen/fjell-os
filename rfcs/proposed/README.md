# RFCs under review

Empty by design when nothing is under review — but this file must exist.

Git does not track empty directories, so without a keeper file this folder
vanishes from a fresh clone. `rfc-status-folder` reads `proposed/`,
`accepted/` and `done/` and **fails closed** when one cannot be read, so an
absent folder turns Gate 12 red for everyone who clones the repository while
looking fine in the working tree that created it.

That happened once, at `d5edf31`: `proposed/` emptied when RFC-0.26-001 moved
to `accepted/`, and the commit that moved it was verified against a working
tree that still had the directory.

`archive/` carries the same kind of stub for the same reason (RFC-0.25-002 R1).
