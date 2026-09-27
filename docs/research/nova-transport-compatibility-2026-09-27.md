<!-- SPDX-License-Identifier: MIT -->
# Nova RPC/transport separation and Caraxes compatibility

Reference: Alexandre Courbot's [public 10-patch series](https://ratatoskr.run/nova-gpu/2026/09/17648741/t),
2026-09-27, Message-ID `20260927-cmdq-rpc-v1-0-822db5af910e@nvidia.com`,
change-id `20260927-cmdq-rpc-ed5f850b207e`. The cover describes separating
transport from RPC in preparation for r000/GMC, with no significant functional
change intended. No upstream code is copied into Caraxes.

## Relevant differences

| Concern | Caraxes evidence / action |
|---|---|
| Queue extent | 570/595 has an outer `elemCount`, currently ignored on receive. 610 has no such field. 615 already uses outer `mctpPayloadSize`. Do not transplant one version's rule into another. |
| RPC header validation | Both receive paths computed whether signature/version permit an error reply, but did not reject otherwise well-formed invalid RPC headers. Add rejection and subsequent-valid-message recovery tests. |
| GMC correlation | The wire sequence is 64 bits. Adapting through `RpcHeader.sequence` loses its upper half. Preserve the native header for both successful and rejected GMC responses. |
| RPC fragmentation | Keep continuation records on the RPC path. A GMC response can occupy several transport slots but is not an RPC continuation series. |
| Integrity and diagnostics | v0 checks a checksum; v2 checks framing and has no checksum. Logs must distinguish these. Bound the outer frame before interpreting nested messages where the format permits it. |
| Shared sequence stream | Add focused mixed RPC/GMC, asynchronous sequence-zero, continuation/interleave and wrap tests. Existing round-trip tests do not establish all combinations. |
| Recovery | Existing malformed-queue and reload tests should be joined into failure → reset/reload → valid-request tests. |

The immediate fixes should stay small and retain the current transport versions.
A later explicit decoded-message type can separate RPC and GMC more completely.
The pre-610 count check belongs to a separate legacy-compatibility change, with
malformed-count and count/length-disagreement tests; it is not a 615 fix.

The series neither demonstrates the Windows Code 43 cause nor predicts a shorter
steady GPU submission path. Its value here is protocol correctness and reducing
coupling between future guest-driver versions. Detailed review and individual
patch links are in `/mnt/vmstore/caraxes/comparison-2026-09-27/nova-cmdq-rpc-review.md`.
