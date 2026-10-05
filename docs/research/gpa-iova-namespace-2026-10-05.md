# Guest GPA and host IOVA separation

The first Windows run after the host reboot (C170) reported Code 43 before GSP
initialization. No benchmark or rendering pass was accepted.

- GPA `[0, 1 GiB)` registered at host IOVA `[2, 3 GiB)`.
- Publication rejected the later GPA `[2, 3 GiB)` identity alias as ambiguous.
- The previously successful C169 run registered every chunk above guest RAM.
- A stock-device restart did not recover C170. Host cleanup completed without Xid.

Host execution now selects GPAs explicitly. CPU accesses use the registered guest
ranges; shadow leaves translate those GPAs to host IOVAs. Numeric overlap between
these namespaces is valid. Failed shadow translation refuses the leaf rather than
retrying its number as a host IOVA. Legacy IOVA guests retain alias validation.

Regression coverage includes distinct page contents, published views, missing
pages, range boundaries, reset, unmap, disconnect, and shadow refusal after an
unmap whose address still equals another page's host IOVA. Async registration
retains the existing pending/applied lifecycle.

Commit `514be02c` passed independent review, 927 unit tests, two integration tests
and a release build; one unit test remains ignored. Stock Windows C173/C174/C175
booted with Problem Code 0 and passed three fresh D3D11 readbacks each. C173 also
completed two locally validated Fire Strike runs; later polling attempts did not
complete every workload and are excluded from performance comparisons.

Subsequent registrations used high IOVAs. Successful boots do not prove that the
low-IOVA case was exercised again. See [the overnight qualification](night-performance-2026-10-06.md).
