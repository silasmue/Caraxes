<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
# Briefing (operator, 2026-09-23): DisplayPort, NVIDIA display engine, Nova compatibility

Source report and implementation briefing handed over by the operator on 2026-09-23 as a
side assignment ("run it in parallel where possible, not top priority, can help us a lot in
the long run"). Kept verbatim below (German). Caraxes context that the briefing did not have:
[display-path.md](../vfio/display-path.md), [display-connector.md](../vfio/display-connector.md),
[display-scaling.md](../vfio/display-scaling.md), decisions D15/D16 and P12 in [README.md](caraxes-record-2026-09-28.md),
[display-d0.md](caraxes-display-d0-2026-09-23.md). Sources S01-S31 are named, not linked; agents resolve them
against `vendor/open-gpu-kernel-modules` (610.57.04) and public trees and record the commits
they used.

---

## 0. Aussagegrenzen und Ausgangspunkt

Dieser Bericht verbindet zwei getrennte Vorhaben: einen virtuellen DisplayPort-Monitor einschließlich NVIDIA-Scanout und einen unabhängigen nova-core-Gasttreiber-Test. Beide sollen das bestehende einzelne vfio-user-nvgpu-Gerät erweitern, nicht durch ein zweites Gastgerät ersetzen. Der Caraxes-Projektstand stammt aus den im Chat bereitgestellten Berichten; das Repository wurde für diese Recherche nicht direkt untersucht. Ein im Quelltext vorhandener Pfad ist kein bestandener Caraxes-Test. Quellenbefund = in öffentlichem Code festgestellt; Vorschlag = abgeleitete Entscheidung; Offen = durch Source-Trace oder Experiment zu klären. Vor der Implementierung die verwendeten Commits und Firmware-Hashes festhalten.

## 1. Zentrale Architekturentscheidung

Display in vier Module schneiden: Display-/AUX-/EDID-Controls -> NVIDIA-Control-Adapter -> DP-Sink; Display-Kanäle/MMIO -> Turing-Displaymodell -> validierter Scanout -> Present-Backend / Host-Viewer. Der DP-Sink beschreibt den Monitor (Anschlusszustand, DPCD, EDID, Link-Training). Das Displaymodell interpretiert die gerätespezifischen Displayoperationen. Das Present-Backend macht das Bild auf dem Host sichtbar. Der Control-Adapter übersetzt die versionierte NVIDIA-ABI in interne Zustände. Ein funktionierender Compute-Pfad ersetzt keinen Modeset: die Display-Class-Header zeigen eigene Kanäle, Methoden und Synchronisation (S08, S09).

## 2. Einstieg: AUX über NVIDIA-RM-Controls

2.1 EvoAuxBus::transaction() in src/common/displayport/src/dp_evoadapter.cpp baut NV0073_CTRL_DP_AUXCH_CTRL_PARAMS und ruft NV0073_CTRL_CMD_DP_AUXCH_CTRL; die NVKMS-Bridge src/nvidia-modeset/src/dp/nvdp-evo-interface.cpp führt rmControl0073()/rmControl5070() auf nvRmApiControl() zurück (S01, S02). Offen: ob jeder Control als GSP_RM_CONTROL am Caraxes-Endpunkt erscheint; den lokalen Dispatch verfolgen und je Control notieren: lokal im Gast, GSP-RPC, MMIO oder Kombination. Keine elektrische AUX-Emulation, bevor die Grenze feststeht.

2.2 ABI-Fallen: NV0073_CTRL_CMD_DP_AUXCH_CTRL = 0x731341; ein Request transportiert höchstens 16 Bytes; size ist im Request Anzahl minus eins, im Reply die tatsächlich übertragene Anzahl (16 Bytes: Request 15, Reply 16). Address-only-I2C ist ein separater Fall (bAddrOnly); NV_OK und AUX-/I2C-ACK/NACK/DEFER sind verschiedene Ergebnisebenen (S03). Im Rust-Kern mit realen Byteanzahlen arbeiten; die NVIDIA-Kodierung nur im ABI-Adapter. Tests: 1 und 16 Bytes, verkürzte Antwort, Address-only, MOT-Fortsetzung, NACK, DEFER, ungültige Länge, ungültige Display-ID. Der Header enthält einen separaten größeren I2C-Transfer-Control; erst die tatsächlich auftretenden Aufrufe inventarisieren.

## 3. EDID ist ein Zustand

3.1 nvDpyReadAndParseEdidEvo() (nvkms-dpy.c) bezieht EDID aus der DP-Library, übergibt sie mit WriteEdidToResman() an RM und liest erneut; SST und MST verschieden; ClearCustomEdid() nutzt einen nullinitialisierten SET_EDID_V2-Payload mit bufferSize=0 (S04). Pro Anschluss sink_edid, override_edid, Cache-Zustand und Connection-State getrennt halten.

3.2 ctrl0073specific.h beschreibt bei SET_EDID_V2 sowohl das Entfernen bei leerem Buffer als auch einen Nullgrößenfall, der die EDID statisch hält; GET_EDID_V2 unterscheidet Cache/DDC und Raw/Cooked (S05). Den tatsächlich verwendeten leeren Clear-Payload testen und dokumentieren. Erster Monitor: eigene EDID mit genau einem bevorzugten 1920x1080@60-Modus, digitaler Eingang, gültige Prüfsumme, keine Audio-/HDR-Erweiterungen; mit einem zweiten Parser gegenlesen.

## 4. Was QEMU liefert

hw/display/dpcd.c: kleines AUX-Slave-Modell mit DPCD-Speicher und synthetischen Link-Statusbits (S11), kein vollständiger Sink. hw/misc/auxbus.c: Native-AUX und I2C-over-AUX-Transaktionen mit Buszustand (S12). hw/display/xlnx_dp.c: Controllerseite (S13). Befund: QEMUs DPCD setzt beim Reset bereits erfolgreiche Lane-Statusbits und kombiniert eine DPCD-1.0-Kennung mit 5,4 Gbit/s, kein konsistentes Profil. QEMU als Referenz für Zustandsverwaltung und Tests, nicht als Autorität; Linux-Registerdefinitionen (S10) und das NVIDIA-Client-Verhalten danebenlegen. GPL-Hinweise der QEMU-Dateien bei Übernahme erfassen; ein Sprachwechsel ist keine neue Lizenz.

## 5. Rust-Referenzen

Kein verifizierter Rust-Port von dpcd.c plus auxbus.c gefunden (Suchergebnis, kein Beweis). Bausteine: redid (EDID-1.3/1.4-Typen, IntoBytes, Rustdoc 0.2.0; S17), Smithay libdisplay-info-rs 0.3.0 (Bindings an die C-Bibliothek zum Gegenprüfen; S18, S19), qemu-display (D-Bus-Viewer-Client mit optionalem DMA-BUF; S20), QEMU-Rust pl011/hpet als Zustandsmodell-Beispiele (S14-S16), rust-vmm/vfio vfio-user Transport (S22). Befund: der geprüfte rust-vmm-Server behandelt GetRegionIoFds, DmaRead, DmaWrite und UserDirtyPages als nicht unterstützt; Sparse-Mapping-Strukturen sind vorhanden. Keine Optimierung auf Doorbell-I/O-FDs ohne Prüfung beider Enden. Empfehlung: den funktionierenden Transport nicht austauschen; den DP-Sink als kleine transportneutrale Rust-Zustandsmaschine ergänzen.

## 6. Minimaler DP-Sink: Vertrag

Externer SST-Monitor, 1080p60, RGB/8 bpc, DP-1.1a-Profil mit höchstens vier Lanes und HBR; kein MST, DSC, HDR, VRR, Audio, HDCP, eDP, Repeater. Fähigkeiten nur melden, wenn passende Antworten existieren. DPCD-Inventur: 0x000.. Revision/Receiver-Caps; 0x100-0x101 Link-Rate/Lanes; 0x102 Training-Pattern; 0x103-0x106 Lane-Training; 0x202-0x207 Lane-Status/Alignment/Adjust; 0x600 Power (S10). Ablauf Disconnected -> Connected -> Training -> Trained; Lane-Status passend zur Konfiguration; ungültige Konfigurationen kontrolliert ablehnen; reservierte/optionale Register nach Semantik. I2C-over-AUX: EDID-Offset, MOT-/Transaktionszustand, Segmentadressierung für Erweiterungsblöcke; Reset setzt einen begonnenen Transfer nicht fort; Hotplug und Link-IRQ getrennt modellieren. Erster hardwarefreier Test: gespeicherte AUX-Requestfolge gegen das Sink-Modell abspielen, exakt dieselben Antworten; ein Test wechselt absichtlich Lane-Konfiguration und EDID-Offset.

## 7. NVIDIA-Displayengine

7.1 nvkms-hal.c ordnet Turing der C5-Familie zu: NVC570_DISPLAY, NVC57D_CORE_CHANNEL_DMA, NVC57E_WINDOW_CHANNEL_DMA; Auswahl anhand der Klassenliste (S06). Verhalten in nvkms-evo3.c; Methoden in clc57d.h und clc57e.h (S07-S09).

7.2 C57E hat eigene DMA-Opcodes, PUT/GET, UPDATE, Semaphore-Acquire/Release, Notifier; Context-DMA-Handles; Format-, Pitch-, Blocklinear-Parameter. Eigener Display-Decoder; Speicherübersetzung als Backend. Normalisierter Scanout-Auftrag: validierte Buffer-ID, Offset, Ausdehnung, Format, Layout, Pitches, Synchronisationsbezüge. Unbekannte Layouts sichtbar ablehnen.

7.3 Getrennte Zustände für eingereichte, wartende und aktive Präsentationen; VBlank-Takt, Render-Abschluss und Buffer-Freigabe sind verschiedene Ereignisse; ein Timer darf keine unerfüllte Acquire-Bedingung erfüllen. Tests: alternierende Bildinhalte; dann eine Acquire-Bedingung absichtlich unerfüllt lassen.

## 8. Hostausgabe ist ein separater Anschluss

QEMUs D-Bus-Displayprotokoll und qemu-display als Referenz (S20, S21). Ein virtueller Head erzeugt keine Cloud-Hypervisor-Anzeige; zuerst ein lokales Present-Backend, das ein validiertes Bild sichtbar macht oder dumpt; DMA-BUF/Zero-Copy erst als bewiesen markieren, wenn es mit dem RM-Backend funktioniert. Keine physischen Host-DP-Anschlüsse durchreichen; das virtuelle Display bleibt vom Host-Desktop unabhängig.

## 9. Nova-core als unabhängiger Gasttreiber-Test

9.1 nova-core ist die GSP-basierte Grundlage für nova-drm und einen vGPU-Manager; die API reicht Firmware-Versionsdetails nicht nach oben durch (S24, S25). Caraxes kann als synthetische Hardware unter Nova dienen, ohne dass der Hosttreiber Nova ist; zwei Gasttreiber gegen dasselbe Geräteprofil sind stärkere Evidenz als zwei OpenRM-Versionen.

9.2 Befunde: gpu.rs enthält TU106 und prüft nach BOOT_0 NV_PMC_BOOT_42 (S26); gsp/hal.rs hat einen Turing-Bootpfad (S29); gsp/fw.rs bindet r570_144 (S28), also keine 610.57.04-Antwortstruktur blind wiederverwenden, versionierten Decoder/Encoder planen; gsp.rs richtet LOGINIT, LOGINTR, LOGRM, RMARGS ein (S27), LIBOS-Argumente über die Kennung finden; Command-Typen unterscheiden Nachrichten ohne Reply von Abfragen (S30); Setup/Cleanup enthalten einen GSP-Unload-Pfad (S26); die Aufgabenliste dokumentiert die instabile GSP-ABI (S31).

9.3 Erster Meilenstein: separater Gast mit festem Kernel-Commit, Konfiguration und Firmware; keine konkurrierenden Treiber an derselben Funktion. Erfolg: Nova erkennt das virtuelle TU106 -> Boot-/Flush-/Registerzustände konsistent -> GSP-Queues mit der gewählten ABI -> GET_GSP_STATIC_INFO liefert Namen und VRAM-Bereiche -> Unload/Reset/erneuter Probe. Kein Nachweis von nova-drm, CUDA oder Display. GSP-ABI- und Bootmodell-Änderungen getrennt von der Displayarbeit integrieren.

## 10. Arbeitsaufteilung und Gates

Pakete: A DP-Sink (EDID, DPCD, AUX, Linkzustand; transportneutral; Rust-Tests); B NVIDIA-Displayadapter (Control-Dispatch, Klassen, C57D/C57E, Notifier; Inventar mit Call-Sites und Trace bis zum ersten fehlenden Verhalten); C Nova-Kompatibilität (ABI, Bootregister, Queues, Cleanup; versionierter Probe-Bericht); D Present-Backend (Bufferauflösung, Format/Layout, Sichtbarkeit; deterministisches Testbild). Der Coordinator besitzt die gemeinsamen Schnittstellen; kein Paket baut nebenbei RM-, UVM- oder Speicherpfad um; der Headless-Modus bleibt reproduzierbar.

Display-Gates: D0 Baseline unverändert; D1 Sink-Unit-Tests; D2 verifizierte Control-/MMIO-Route; D3 im Gast korrekt erkannter Monitor mit richtigen Modi; D4 sichtbares Testbild; D5 alternierende Flips samt absichtlich verzögerter Synchronisation; D6 Abstecken, Reset, Wiederanstecken. Nova-Gates: N0 Build und Firmware gepinnt; N1 Geräteerkennung; N2 GSP-Boot/Queues; N3 statische Daten; N4 Cleanup und Wiederholung. Jeder Lauf archiviert Commit, Konfiguration, ABI, Profil, letzte erfolgreiche Operation, ersten Fehler, minimalen Trace.

## 11. Sicherheits- und Integrationsregeln

Nur ein gast-sichtbares NVIDIA-Gerät; keine dauerhaften Gasttreiberpatches; keine Änderungen am physischen Hosttreiber; Tests gegen echte Hardware nur mit den freigegebenen Harnesses; keine ungefragten Host-Resets oder Display-Disconnects. Alle guestkontrollierten Längen, Offsets, Handles, DMA-Ziele und Methodenzahlen prüfen; Decoder mit Arbeitsbudget; keine ungeprüften Gastadressen als Hostadressen. Scanout, Notifier und Semaphore gehören in dieselbe Eigentumsprüfung wie Compute-Buffers.

## 12. Offene Fragen

Erreichbare Display-Controls, minimale Display-RPC-Liste, gewählte Connector-/SOR-Topologie, Interrupt-Routing, erster realer Scanout-Speicherpfad, Synchronisation mit dem Host-Viewer, die Nova-ABI-Differenz byteweise. Erste Entscheidungen: Wo terminiert AUX? Welche Display-Kanäle werden benutzt? Wie wird ihr Speicher validiert? Welcher Nova-Quellstand ist das Ziel? Konfidenz 94 % für die Schnittstellengrenzen; keine quantitative Erfolgswahrscheinlichkeit.

## 13. Quellen (Namen, wie übergeben)

S01 NVIDIA 610.57.04 DP AUX adapter / EvoAuxBus::transaction · S02 NVKMS-RM interface · S03 DisplayPort control ABI (ctrl0073dp.h) · S04 EDID lifecycle in NVKMS (nvkms-dpy.c) · S05 display-specific controls / EDID (ctrl0073specific.h) · S06 display HAL and class selection (nvkms-hal.c) · S07 nvEvoC5 (nvkms-evo3.c) · S08 clc57d.h · S09 clc57e.h · S10 Linux DP/DPCD register definitions (include/drm/display/drm_dp.h) · S11 QEMU hw/display/dpcd.c · S12 QEMU hw/misc/auxbus.c · S13 QEMU hw/display/xlnx_dp.c · S14 QEMU Rust development documentation · S15 QEMU Rust pl011 · S16 QEMU Rust hpet · S17 redid 0.2.0 · S18 Smithay libdisplay-info-rs · S19 libdisplay-info 0.3.0 · S20 qemu-display README · S21 QEMU D-Bus display protocol · S22 rust-vmm/vfio vfio-user · S23 vfio-user protocol specification · S24 Linux Nova overview · S25 Nova firmware API guidelines · S26 nova-core gpu.rs · S27 nova-core gsp.rs · S28 nova-core gsp/fw.rs · S29 nova-core gsp/hal.rs · S30 nova-core typed GSP commands · S31 Nova task list.
