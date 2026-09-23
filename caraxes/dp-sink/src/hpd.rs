// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
// SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
//! Hot-plug detect: the level (plugged or not) and the pulses a source sees.
//!
//! A long pulse is a level change (plug or unplug; a replug is an unplug followed by a
//! plug). A short pulse (IRQ_HPD, 0.5-1 ms low) keeps the level high and tells the source
//! to read the sink's status registers. The two are separate event kinds so a transport
//! can map them onto different notifications (for NVIDIA: the hotplug event versus the DP
//! IRQ notifier).

use std::collections::VecDeque;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HpdLevel {
    Low,
    High,
}

/// Why the sink raised a short pulse.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IrqCause {
    /// Link status changed (0x202-0x205, `LINK_STATUS_UPDATED`).
    LinkStatus,
    /// `DEVICE_SERVICE_IRQ_VECTOR` bits were set.
    DeviceService(u8),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HpdEvent {
    /// Long pulse: level went high.
    Plug,
    /// Long pulse: level went low.
    Unplug,
    /// Short pulse while high.
    Irq(IrqCause),
}

impl HpdEvent {
    pub fn is_long(self) -> bool {
        matches!(self, HpdEvent::Plug | HpdEvent::Unplug)
    }
}

#[derive(Clone, Debug)]
pub struct Hpd {
    level: HpdLevel,
    pending: VecDeque<HpdEvent>,
}

impl Default for Hpd {
    fn default() -> Self {
        Self::new()
    }
}

impl Hpd {
    pub fn new() -> Self {
        Hpd {
            level: HpdLevel::Low,
            pending: VecDeque::new(),
        }
    }

    pub fn level(&self) -> HpdLevel {
        self.level
    }

    /// Raise the level. Returns false (no event) if it was already high.
    pub fn plug(&mut self) -> bool {
        if self.level == HpdLevel::High {
            return false;
        }
        self.level = HpdLevel::High;
        self.pending.push_back(HpdEvent::Plug);
        true
    }

    /// Drop the level. Returns false (no event) if it was already low. Pending short pulses
    /// are discarded: a source cannot service an IRQ of an unplugged sink.
    pub fn unplug(&mut self) -> bool {
        if self.level == HpdLevel::Low {
            return false;
        }
        self.level = HpdLevel::Low;
        self.pending.retain(|e| e.is_long());
        self.pending.push_back(HpdEvent::Unplug);
        true
    }

    /// Short pulse. Returns false (dropped) while the level is low.
    pub fn irq(&mut self, cause: IrqCause) -> bool {
        if self.level == HpdLevel::Low {
            return false;
        }
        self.pending.push_back(HpdEvent::Irq(cause));
        true
    }

    /// True if a short pulse is waiting to be delivered.
    pub fn irq_pending(&self) -> bool {
        self.pending.iter().any(|e| !e.is_long())
    }

    pub fn pending(&self) -> impl Iterator<Item = &HpdEvent> {
        self.pending.iter()
    }

    /// Hand all pending events to the transport, oldest first.
    pub fn take(&mut self) -> Vec<HpdEvent> {
        self.pending.drain(..).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn long_and_short_pulses_are_separate() {
        let mut h = Hpd::new();
        assert!(!h.irq(IrqCause::LinkStatus), "no IRQ while low");
        assert!(h.plug());
        assert!(!h.plug(), "plug is idempotent");
        assert!(!h.irq_pending());
        assert!(h.irq(IrqCause::DeviceService(0x02)));
        assert!(h.irq_pending());
        assert_eq!(
            h.take(),
            [HpdEvent::Plug, HpdEvent::Irq(IrqCause::DeviceService(0x02))]
        );
        assert!(h.irq(IrqCause::LinkStatus));
        assert!(h.unplug());
        assert_eq!(h.take(), [HpdEvent::Unplug], "unplug discards pending IRQs");
        assert_eq!(h.level(), HpdLevel::Low);
    }
}
