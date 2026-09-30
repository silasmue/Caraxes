<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
# dp-sink: a transport-neutral DisplayPort SST sink

Package A of the display side assignment ([briefing](../../docs/history/caraxes-briefing-display-nova-2026-09-23.md),
P16 in [docs/vfio/README.md](../../docs/history/caraxes-record-2026-09-28.md)). This crate models the monitor
end of an emulated DisplayPort connector: the DPCD register file, the link-training state
machine, the AUX engine with I2C-over-AUX, the EDID and hot-plug detect. It knows nothing about
NVIDIA, vfio-user or GSP. The NVIDIA control adapter (package B) and the display model will sit
on top of it (decision D15: DisplayPort SST, one connector per head).

Status 2026-09-23: written and unit-tested (gate D1, "Sink-Unit-Tests"). Moved from
`caraxes/dp-sink` to `crates/dp-sink` as a workspace member (still `std` only) for display
phase B ([docs/vfio/display-ab.md](../../docs/vfio/display-ab.md)). There it sits behind
the device's NV0073 AUX adapter (`crates/vfio-user-nvgpu/src/display/aux.rs`) and answered the
stock 610.57.04 guest's DP library: detection, DisplayID probe, EDID read, HBR3 x4 training
through `DP_CTRL`. The golden transcript is now that guest's recorded capture.

```sh
cargo test -p dp-sink                                 # 52 tests
cd crates/dp-sink
cargo clippy --all-targets -- -D warnings
cargo run -q --example edid | edid-decode --check     # EDID conformity: PASS
cargo run -q --example edid -- --cta | edid-decode --check
```

## 1. Contract (briefing section 6)

An external SST monitor, 1920x1080 at 60 Hz, RGB at 8 bpc. DPCD 1.1a profile with at most four
lanes and HBR. No MST, DSC, HDR, VRR, audio, HDCP, eDP or repeaters in the default profile.
A capability is reported only if the sink has matching answers for it.

What the crate provides:

| Module | Content |
|---|---|
| `profile` | `SinkProfile`: every capability as data. `dp11a_hbr_x4()` is the default, `dp14_hbr3_x4_dsc()` is the D15 headroom profile. `validate()` rejects inconsistent profiles, for example HBR2 without DPCD 1.2 and TPS3, or DSC below 1.4 |
| `dpcd` | `Dpcd`: register reads and writes with defined semantics, and the link-training state machine driven by those writes |
| `link` | `LinkConfig`, `LinkState`, `TrainingPattern`, `LinkError` (why a configuration is refused) |
| `aux` | `AuxRequest` (on real byte counts), `AuxReply` (reply code plus bytes actually transferred), `AuxError` (no transaction took place), `Fault` (injected replies) |
| `ddc` | the I2C bus behind I2C-over-AUX: the EDID EEPROM at 0x50 and the E-DDC segment pointer at 0x30 |
| `edid` | EDID 1.4 generator, a CVT reduced-blanking calculator, an optional CTA-861 extension |
| `hpd` | HPD level and events: long pulses (plug, unplug) and short pulses (IRQ) are separate kinds |
| `sink` | `DpSink` (one monitor behind `transact()`), `SinkPorts` (several sinks by port id) |
| `transcript` | a line format for AUX sequences, `replay()` (checks every reply exactly), `record()` |

Entry points of `DpSink`:

```rust
fn new(profile: SinkProfile, edid: Vec<u8>) -> Result<DpSink, SinkError>;
fn transact(&mut self, req: &AuxRequest) -> Result<AuxReply, AuxError>;
fn plug(&mut self) -> bool;  fn unplug(&mut self) -> bool;  fn set_edid(&mut self, edid) -> Result<..>;
fn train(&mut self, cfg: LinkConfig) -> Result<(), LinkError>;   // for DP_CTRL-style transports
fn drop_link(&mut self) -> bool;  fn service_irq(&mut self, bits: u8) -> bool;
fn take_events(&mut self) -> Vec<HpdEvent>;
fn aux_reset(&mut self);  fn inject(&mut self, fault: Fault);
```

## 2. Profile and DPCD

### Receiver capabilities (0x000-0x00F)

| Addr | Name | Default (1.1a, HBR x4) | Headroom (1.4, HBR3 x4, DSC) |
|---|---|---|---|
| 0x000 | DPCD_REV | 0x11 | 0x14 |
| 0x001 | MAX_LINK_RATE | 0x0A (HBR) | 0x1E (HBR3) |
| 0x002 | MAX_LANE_COUNT | 0x84 (4 lanes, enhanced framing) | 0xC4 (and TPS3) |
| 0x003 | MAX_DOWNSPREAD | 0x01 (0.5 %) | 0x81 (and TPS4) |
| 0x004 | NORP | 0x00 (one receiver port) | 0x00 |
| 0x005 | DOWNSTREAMPORT_PRESENT | 0x00 (no branch) | 0x00 |
| 0x006 | MAIN_LINK_CHANNEL_CODING | 0x01 (8b/10b) | 0x01 |
| 0x007 | DOWN_STREAM_PORT_COUNT | 0x00 (no OUI support) | 0x00 |
| 0x008 | RECEIVE_PORT0_CAP_0 | 0x02 (local EDID present) | 0x02 |
| 0x009 | RECEIVE_PORT0_CAP_1 | 0x00 (32-byte buffer) | 0x00 |
| 0x00E | TRAINING_AUX_RD_INTERVAL | 0x00 | 0x80 (extended caps at 0x2200) |
| others | | 0x00 | 0x00 |

At DPCD 1.4 the extended field 0x2200-0x220F repeats 0x000-0x00F. At DPCD 1.2 and later the
event status indicator mirrors are present: 0x2002 = SINK_COUNT, 0x2003 = the service IRQ
vector (write 1 to clear), 0x200C-0x200F = 0x202-0x205. DSC caps (0x060-0x06F) are a 16-byte
array in the profile. FEC_CAPABILITY is 0x090. MSTM_CAP (0x021) is 0 in both profiles.

### Register semantics

| Class | Addresses | Read | Write |
|---|---|---|---|
| Capabilities | 0x000-0x00F, 0x060-0x06F, 0x090, 0x2200-0x220F | profile | acknowledged and ignored |
| Link configuration | 0x100 LINK_BW_SET, 0x101 LANE_COUNT_SET (bits 7, 4:0), 0x102 TRAINING_PATTERN_SET, 0x103-0x106 TRAINING_LANEx_SET (bits 5:0), 0x107 DOWNSPREAD_CTRL (bits 7, 4), 0x108 MAIN_LINK_CHANNEL_CODING_SET (bit 0) | stored value | stored (defined bits only); 0x100-0x106 drive training |
| Profile-gated | 0x120 FEC_CONFIGURATION, 0x160 DSC_ENABLE | stored if offered, else 0 | stored if offered, else ignored |
| Status | 0x200 SINK_COUNT, 0x202-0x207 lane status, align, sink status, adjust requests | computed | acknowledged and ignored |
| Write 1 to clear | 0x201 DEVICE_SERVICE_IRQ_VECTOR (and 0x2003) | pending bits | clears the written bits |
| Source OUI | 0x300-0x302 | stored | stored |
| Power | 0x600 SET_POWER | stored | D0 (1) and D3 (2), plus D3 AUX-on (5) at 1.4; reserved values are ignored |
| Everything else, 0x00000-0xFFFFF | | 0 | acknowledged and ignored |

What the model decided:

- **Undefined and reserved registers read 0.**
- **A write to a read-only or reserved register is ACKed with the full byte count and has no effect.** It is never NACKed. A native NACK means the sink did not accept the request, and on a write it reports a partial count. A burst that crosses a read-only byte would then become a partial write for the source to repair. See ambiguity 1 below.
- **An unsupported value in a writable register is stored as written.** The refusal appears when training starts: the link state becomes `Refused(reason)` and the lane status stays 0, which is how a real source finds out (by polling 0x202-0x207).
- **A native request that runs past 0xFFFFF is NACKed** with count 0. A request whose start address does not fit 20 bits is the transport error `InvalidAddress`.

### Link-training state machine

```
            plug                      TPS1 (valid config)
Disconnected ----> Connected ------------------------------> Training(TPS1)
     ^   unplug       ^  ^  \  TPS1/2/3/4 with invalid config        |
     +----------------+  |   +------------> Refused(reason) <--------+ TPS2/3/4 without CR
          (any state)    |                     |   pattern 0          |   on the same config
                         |  <------------------+                      v
                         |  pattern 0 during TPS1          Training(TPS2 | TPS3 | TPS4)
                         |  config change while Trained              |  pattern 0
                         |  SET_POWER != D0                           v
                         |  sink link loss (IRQ_HPD)               Trained
                         +--------------------------------------------+
```

- **Clock recovery (TPS1):** 0x100/0x101 are checked against the profile. The rate must be an 8b/10b code no higher than MAX_LINK_RATE. The lane count must be 1, 2 or 4 and no higher than MAX_LANE_COUNT. Enhanced framing needs the capability. If the check passes, lanes `0..n` report CR_DONE (per-lane nibble 0x1) and lanes `n..4` report 0.
- **Equalization (TPS2, TPS3, TPS4):** the pattern must exist at this DPCD revision and in the profile. TPS3 needs revision 1.2 and the TPS3 bit; TPS4 (code 7, 4-bit field) needs revision 1.4 and the TPS4 bit. Clock recovery must have run with the same configuration. On success the active lanes report CR, EQ and symbol lock (nibble 0x7) and 0x204 reports INTERLANE_ALIGN_DONE.
- **Pattern 0** after equalization means Trained, with SINK_STATUS bit 0 set and the adjust requests cleared.
- **Adjust requests (0x206/0x207)** mirror the swing and pre-emphasis the source wrote to 0x103-0x106, so the sink never asks for a change. The model accepts any drive level.
- **The link is lost** when 0x100/0x101 change value while Trained (rewriting the same values keeps the link), when SET_POWER leaves D0, and on `drop_link()`. `drop_link()` also sets LINK_STATUS_UPDATED (0x204 bit 7) and raises an IRQ_HPD. The next TPS1 clears the bit.
- **Training while not in D0** is refused with `NotPowered`.
- **`DpSink::train(cfg)`** drives the same writes internally: TPS1, then the best EQ pattern the profile offers, then 0. It is meant for transports where training happens behind a control (NVIDIA `DP_CTRL`) instead of AUX writes, so both routes leave the DPCD in the same state.

## 3. AUX engine

`AuxRequest { cmd, addr, len, data }` uses real byte counts. The request command nibble is the DP
encoding: bit 3 native, bit 2 MOT, bits 1:0 write/read/write-status.

The result has two levels:

1. `Err(AuxError)` means no AUX transaction took place:
   - `InvalidLength`: a native request of 0 bytes, more than 16 bytes, or a payload that does not match `len`.
   - `InvalidAddress`: the address does not fit 20 bits (native) or 7 bits (I2C).
   - `Timeout`: HPD is low, or an injected timeout.
   - `NoSuchPort`: `SinkPorts` has no sink at that id.

   Validation comes first, and faults are not consumed by requests that never became transactions.
2. `Ok(AuxReply { code, count, data })` means the sink answered. `code` is one of ACK, NACK, DEFER, I2C_NACK or I2C_DEFER, with the numeric values of the DP reply field (0, 1, 2, 4, 8). Those values are also NV0073 `DP_AUXCH_REPLYTYPE`. `count` is the number of bytes actually transferred: bytes returned for a read, which can be fewer than requested; bytes accepted for a write, which is the "M" of a NACKed write.

Faults are queued with `inject()` and consumed one per valid request: `AuxDefer`, `AuxNack`,
`I2cDefer`, `I2cNack` and `Timeout`. An I2C fault on a native request becomes the AUX-level
reply. A deferred request changes no state, so a retry gets what the first attempt would have
got.

### I2C-over-AUX and DDC

- **START and STOP:** a request whose address or direction differs from the open transaction issues a (repeated) START. An absent device, or a read of the write-only segment pointer, NACKs the address phase (I2C_NACK, count 0), which also ends the transaction. With MOT = 0 the request ends with STOP; with MOT = 1 the transaction stays open, and the next request to the same address and direction continues it.
- **Address-only requests** (I2C with `len == 0`): with MOT they are START plus address, without MOT they are START, address and STOP. The reply is ACK with count 0 if the device is present, I2C_NACK if not.
- **Segment pointer (0x30):** writing one byte selects a 256-byte segment. **Every STOP resets it to 0** (E-DDC), so it applies only within the transaction that wrote it. A reply for a segment that does not exist is I2C_NACK.
- **EEPROM offset (0x50):** writing one byte sets the word offset. **The offset survives STOP**, so a current-address read continues. Bus reset (plug, unplug, `aux_reset()`) returns it to 0.
- **Write-protected EEPROM:** a write of more than one byte to 0x50 or 0x30 gets I2C_NACK with count 1, because only the pointer byte is accepted.
- **Reads** return at most `profile.max_i2c_reply` bytes (1-16, default 16). They also stop at the end of the EDID data and at the end of the segment, so the reply can be short. A read that starts past the end gets I2C_NACK.
- **Write status update requests** reply ACK with count 0 (writes complete immediately). Without MOT they end the transaction.
- **`aux_reset()`** abandons an open transaction and clears segment and offset. A transfer begun before the reset is not continued. DPCD state is untouched.

## 4. EDID

`EdidSpec::first_monitor().build()` produces 128 bytes:

- EDID 1.4, manufacturer `CRX` (synthetic; not a registered PNP id), product 1, week 39 of 2026.
- Video input 0xA5: digital, 8 bpc, DisplayPort. RGB 4:4:4 only, sRGB default, gamma 2.2, 53 x 30 cm.
- Preferred timing (DTD 1): **1920x1080 at 59.934 Hz, CVT-RB**, 138.50 MHz. H 1920/48/32/80, V 1080/3/5/23, +H -V, 527 x 296 mm.
- Established timings 640x480 at 60 (the fail-safe mode DP requires), 800x600 at 60 and 1024x768 at 60. Standard timings 1280x720, 1280x1024, 1440x900, 1600x900 and 1680x1050, all at 60 Hz.
- Monitor name `Caraxes DP`. Range limits (range-limits-only form): 56-61 Hz V, 30-70 kHz H, 150 MHz. A dummy descriptor.
- Extension count 0, valid checksum.

`DetailedTiming::cvt_rb(h, v, hz)` implements the CVT 1.2 reduced-blanking v1 formula in integer
arithmetic. The tests compare it with the published values for 1920x1080, 1920x1200, 2560x1440,
1440x900, 1680x1050 and 2560x1600. The CTA-861 extension (`CtaExtension::basic()`, off by
default) carries two blocks: a video data block with VICs 16, 4 and 1, and a video capability
data block (RGB quantization selectable, always underscanned). It has no audio, HDR or HDMI
blocks.

The generator is not its own oracle. Two independent checks cover it:

- **edid-decode 1.32.0** (host, `/usr/bin/edid-decode`) with `--check`:
  - Base block: `EDID conformity: PASS` with no warnings.
  - With the CTA extension: PASS with one warning, "VIC 16 and the first DTD are not identical". That warning is intended: the native mode is CVT-RB, which no VIC describes, and edid-decode compares the first VIC with the first DTD whatever the order.
  - The test `edid_oracle.rs` runs the tool when it is on `PATH` (or at `EDID_DECODE`), and prints that it skipped otherwise.
- **An independent parser in `tests/edid_oracle.rs`**, written from the EDID 1.4 layout, shares no code with the generator. It checks the header and checksum and re-derives the detailed timing. It compares against the published CVT-RB constants, not the crate's calculator. It also decodes the name, the range limits and the standard timings, and walks the CTA data block collection.

Both oracles read the EDID over I2C-over-AUX the way the NVIDIA DP library does.

## 5. Hot-plug detect

`HpdLevel` (Low, High) and pending `HpdEvent`s are separate:

- `Plug` and `Unplug` are long pulses (level changes). `set_edid()` while plugged produces `Unplug` followed by `Plug`.
- `Irq(LinkStatus)` and `Irq(DeviceService(bits))` are short pulses. They exist only while High; unplugging discards any short pulses not yet delivered.

The transport drains events with `take_events()`.

## 6. How the NVIDIA adapter will map NV0073 onto this crate (future, package B)

The adapter owns the NVIDIA encoding; this crate never sees it. From `ctrl0073dp.h` and
`dp_evoadapter.cpp` `EvoAuxBus::transaction()` (open-gpu-kernel-modules 610.57.04, commit
e4a5faa2567f28c8eabe0ebb6422b6d0abcf37eb):

| NV0073 `DP_AUXCH_CTRL` (0x731341) | dp-sink |
|---|---|
| `displayId` | port id in `SinkPorts`. An unknown id is `AuxError::NoSuchPort`, which the adapter returns as an RM error status, not as a reply type |
| `cmd` bits 3:0 (TYPE, I2C_MOT, REQ_TYPE) | `AuxCommand::from_code(cmd & 0xF)`: the bit layout is the DP request nibble |
| `addr` (20:0) | `AuxRequest.addr` |
| `size` in the request (**count minus one**) and `bAddrOnly` | `len = 0` if `bAddrOnly`; otherwise `len = size + 1`. For a write, `data[..len]` |
| `replyType` | `AuxReply.code.code()` (the same numbers: ACK 0, NACK 1, DEFER 2, I2CNACK 4, I2CDEFER 8) |
| `size` in the reply (**actual count**) | `AuxReply.count` (for 16 bytes: request 15, reply 16) |
| `data` in the reply | `AuxReply.data` |
| `AuxError::InvalidLength` / `InvalidAddress` | status `NV_ERR_INVALID_ARGUMENT`; `replyType` `INVALID_ARGUMENT` (0xFFFFFFFF) |
| `AuxError::Timeout` | status `NV_ERR_TIMEOUT` (the library turns it into a defer and retries) or `replyType` TIMEOUT (3); to be decided against the real RM behaviour |
| `retryTimeMs` | always 0 (no eDP panel power sequencing) |

The other controls:

- **`DP_CTRL` (link training):** `DpSink::train(LinkConfig)`. `LinkError` becomes the control's failure status.
- **Hotplug:** `HpdEvent::Plug` and `Unplug` become the `POST_EVENT` HOTPLUG event. `HpdEvent::Irq` becomes `NV2080_NOTIFIERS_DP_IRQ`.
- **`GET_CONNECT_STATE`:** `DpSink::is_plugged()`.
- **EDID controls:** `SET_EDID_V2` and `GET_EDID_V2` hold the RM-side EDID cache, which is separate state in the adapter (briefing 3.1). On SST the DP library reads the EDID through AUX, which is this crate.

Known encoding trap: the library's DDC ping (`sstDDCPing`, `dp_sst_edid.cpp`) sends an I2C
**read** with `sizeRequested = 0`. `bAddrOnly` is set only for writes, so the control arrives with
`size = 0` and `bAddrOnly = false`. That is indistinguishable from a 1-byte read, and the adapter
must decode it as one. With this sink the ping succeeds, returns EDID byte 0 and advances the
EEPROM offset by one. `readNextBlock` writes the offset before reading, so this is harmless. The
golden transcript records the ping as `ir 50 1`. Whether RM on GSP forwards the control to the
device at all (the NVOC export flags are 0x844 in `g_disp_objs_nvoc.c`; the implementation is not
in the open tree) is package B's question.

## 7. Tests (52)

| File | Count | Content |
|---|---|---|
| `src/profile.rs` | 3 | built-in profiles validate; inconsistent ones are refused; rate codes |
| `src/link.rs` | 2 | configuration validation against the profile; payload bandwidth |
| `src/dpcd.rs` | 11 | caps bytes for both profiles; undefined reads are 0 and read-only writes are ignored; defined-bit masks; W1C IRQ vector; 4-lane training sequence with adjust mirroring; lane status for 1, 2 and 4 lanes; refusals (rate, lanes, TPS3 at 1.1, EQ without CR, lane change during EQ); TPS3/TPS4 on the headroom profile; reconfiguration and power-down; link loss |
| `src/ddc.rs` | 5 | offset then read; short read at the end, NACK past it; segment pointer until STOP; absent devices and extra bytes; max reply |
| `src/aux.rs` | 2 | command nibble round trip; validation rules |
| `src/hpd.rs` | 1 | long and short pulses are separate |
| `src/edid.rs` | 4 | CVT-RB against published values; base block layout; CTA extension; bad inputs |
| `src/transcript.rs` | 1 | request text round trip (the replayer's `! train` action is exercised by `tests/replay.rs`) |
| `tests/aux_cases.rs` | 16 | briefing 2.2 and 6: native read of 1 and 16 bytes, native write of 1 and 16 bytes, I2C read of 1 and 16 bytes, **short reply** (profile limit and EDID end), **address-only**, **MOT continuation** with segment pointer, **NACK** (natural and injected), **DEFER** (state unchanged, retry correct), **invalid length**, **invalid port / display id**, timeout when unplugged, **reset abandons a transfer**, write status request, read-only writes ACKed, direct training API, hotplug vs IRQ events |
| `tests/replay.rs` | 3 | the **golden transcript** (`tests/transcripts/nvidia-610-guest-capture.txt`, 85 request/reply pairs recorded from the stock 610.57.04 guest on 2026-09-23, against `SinkProfile::dp14_hbr3_x4_dsc` and `caraxes-dp-monitor.edid.bin`) replays exactly; the replayer detects one wrong byte; **changed lane count and EDID offset**: the capture trained with 2 lanes (`! train 1e 2 ef`) or with the EDID offset 0x40 must fail at the right line; corrected expectations pass; per-lane status and offset checked against values derived in the test |
| `tests/edid_oracle.rs` | 4 | independent parser re-derives the first monitor read over AUX; CTA block walk; edid-decode on the base block and the CTA variant |

About the golden transcript: until 2026-09-23 it was written by hand from the NVIDIA DP
library's source. It is now the recorded capture of display run b3
(`caraxes/runs/display-ab-2026-09-23/transcript.py` turns the device trace into this format).
The capture corrected the hand-written assumptions:
- the library reads 0x00E and then the DPCD 1.4 extended caps at 0x2200;
- it touches the DP-tunneling registers 0xE000D/0xE0030;
- it sends no DDC ping;
- it probes DisplayID at 0x52 twenty times before reading the EDID;
- it trains through `DP_CTRL`, not over AUX. The new `! train` action replays that.

## 8. Ambiguities the specification left open (decisions taken here)

The VESA DP and E-DDC texts were not available in this session. The sources consulted were
Linux `drm_dp.h`, QEMU, and the NVIDIA DP library. The points below are ambiguous or silent
in those sources, so each is a decision taken here. A real monitor's DPCD dump or the
specification text can overturn any of them.

1. **Writes to read-only and reserved DPCD addresses:** answered with ACK and ignored, not NACK. Reserved bytes reading 0 is common ground; how a sink answers a write to them is not stated in the sources consulted, and QEMU simply stores the byte.
2. **Short replies to native reads:** not generated. Whether a sink may return fewer bytes than requested for a native read is unclear. Drivers (Linux `drm_dp_dpcd_read` callers, the NVIDIA library) treat it as an error, so the knob exists only for I2C reads, where partial replies are explicitly allowed.
3. **EEPROM reads past the end of the EDID:** real monitors wrap, repeat or return 0xFF. This sink returns a short reply and then I2C_NACK. No source reads past the extension count, so it does not matter in practice. The choice keeps "no data" visible.
4. **Offset and segment across STOP:** the offset persists (real EEPROM current-address reads). The segment pointer resets at STOP (the E-DDC rule). A source that relies on the offset being reset after STOP would see a difference.
5. **LINK_STATUS_UPDATED (0x204 bit 7):** set on sink-initiated link loss. The text is unclear about when it clears. Here it clears at the next TPS1, not on read, so reads have no side effects and transcripts stay replayable.
6. **Changing 0x100/0x101 on a trained link:** treated as leaving the trained state, except when the same values are rewritten (a source may rewrite them at every training start). A changed configuration while TPS1 is still set simply re-runs clock recovery for the new configuration.
7. **SET_POWER D3 and AUX:** AUX keeps answering in D3 (a real sink may be slow to wake, or miss the first request). Training is refused until D0. D3 AUX-on (5) is accepted only at DPCD 1.4, because I could not pin the revision that introduced it.
8. **Drive levels:** the sink accepts any swing and pre-emphasis and mirrors them back as its adjust request. Real sinks may ask for more.
9. **DSC headroom bytes** (0x060-0x06F) are plausible DSC 1.2 values (1/2/4 slices, 8 and 10 bpc, 2560-pixel slices). They are data, not checked against any real decoder or against the NVIDIA DP library's DSC logic.
10. **I2C write status without an outstanding write** replies ACK with count 0.
11. **The NV0073 DDC ping encoding** (section 6) makes a zero-size I2C read indistinguishable from a 1-byte read at the control boundary.

## 9. References and licence facts

Everything here was written for this crate. No code was copied or translated from any of the
sources below. Only semantics were taken from reading them.

| Source | Version used | Licence | Use |
|---|---|---|---|
| QEMU `hw/display/dpcd.c`, `hw/misc/auxbus.c`, `include/hw/misc/auxbus.h`, `include/hw/display/dpcd.h` | qemu/qemu master 014b1f730c0faa08067f8e9b5829e62ea3d4b177 (2026-09-23), fetched into the session scratchpad | GPL-2.0-or-later (GreenSocs Ltd 2015) | reading only: MOT restart rules, reply enum. As the briefing noted, its DPCD resets to trained lane status and pairs DPCD 1.0 with 5.4 Gbit/s; this crate does neither. Rewriting GPL code in another language would not change its licence, so none was taken |
| Linux `include/drm/display/drm_dp.h` | v6.12 release tree in the session scratchpad | permissive (Keith Packard 2008, historical MIT-style notice) | register addresses and bit names, checked against the constants in `dpcd.rs` |
| NVIDIA open-gpu-kernel-modules `ctrl0073dp.h`, `dp_evoadapter.cpp`, `dp_sst_edid.cpp`, `dp_edid.h` | 610.57.04, commit e4a5faa2567f28c8eabe0ebb6422b6d0abcf37eb | MIT / GPL-2.0 dual | NV0073 encoding for section 6; request order of the golden transcript |
| redid (crates.io) | 0.2.0 | crates.io metadata "non-standard"; the GitHub repository mripard/redid (HEAD 4956848250954bafb57857fa5455a34563cb61cb) carries an MIT LICENSE | reference only, not a dependency |
| libdisplay-info (crates.io, Smithay bindings) | 0.3.0 | MIT | reference only, not a dependency; it needs the C library, and edid-decode was available as the external oracle |
| edid-decode | 1.32.0 (Arch package, `/usr/bin/edid-decode`) | MIT | external oracle in the tests |

Dependency choice: **`std` only.** Neither crate is clearly better for this job. redid would
generate the EDID but then could not serve as an independent oracle, and its licence metadata
is inconsistent. libdisplay-info adds a C dependency to a crate that must stay small and
transport-neutral.
