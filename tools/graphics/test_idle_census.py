# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
"""CPU-only accounting oracles for idle-census.py."""

import importlib.util
import unittest
from pathlib import Path


spec = importlib.util.spec_from_file_location("idle_census", Path(__file__).with_name("idle-census.py"))
census = importlib.util.module_from_spec(spec)
spec.loader.exec_module(census)


class AccountingTests(unittest.TestCase):
    def test_guest_ticks_are_not_added_twice_and_aggregate_is_per_cpu(self):
        a = census.parse_cpus("cpu 0 0 0 0 0 0 0 0 0 0\ncpu0 0 0 0 0\n")
        b = census.parse_cpus("cpu 200 0 0 200 0 0 0 0 100 0\ncpu0 100 0 0 100\n")
        result = census.cpu_delta(a, b, seconds=2, hz=100)
        self.assertEqual(sum(result["cpu"]["ticks"].values()), 400)
        self.assertEqual(result["cpu"]["percent_of_one_cpu"]["user"], 100)
        self.assertEqual(result["cpu0"]["percent_of_one_cpu"]["idle"], 50)

    def test_cpu_regression_is_explicit_instead_of_clamped(self):
        a = census.parse_cpus("cpu 0 0 0 100 9 0 0 0")
        b = census.parse_cpus("cpu 0 0 0 200 8 0 0 0")
        result = census.cpu_delta(a, b, 1, 100)["cpu"]
        self.assertEqual(result["regressed_fields"], ["iowait"])
        self.assertIsNone(result["percent_of_one_cpu"])

    def test_process_names_with_spaces_and_parentheses(self):
        # Linux stat fields 3 through 22; utime=7, stime=5, starttime=123.
        tail = ["S"] + ["0"] * 19
        tail[11], tail[12], tail[19] = "7", "5", "123"
        result = census.parse_process("42 (a (worker) name) " + " ".join(tail))
        self.assertEqual(result["name"], "a (worker) name")
        self.assertEqual((result["user_ticks"], result["system_ticks"], result["start_ticks"]),
                         (7, 5, 123))

    def test_pid_reuse_and_process_thread_overlap(self):
        def row(kind, start, user):
            return dict(kind=kind, start_ticks=start, user_ticks=user, system_ticks=0)
        a = {"rows": {"p1": row("process", 1, 10), "p2": row("process", 2, 0),
                      "t1": row("thread", 1, 10)}}
        b = {"rows": {"p1": row("process", 1, 110), "p2": row("process", 3, 200),
                      "t1": row("thread", 1, 110)}}
        result = census.process_delta(a, b, 2, 100, 20)
        self.assertEqual(result["replaced"], ["p2"])
        self.assertEqual(result["matched_processes"], 1)
        self.assertEqual(result["matched_threads"], 1)
        self.assertEqual(result["top_processes"][0]["percent_of_one_cpu"], 50)
        self.assertEqual(result["top_threads"][0]["percent_of_one_cpu"], 50)

    def test_irq_shapes_and_missing_sources(self):
        a = {"rows": census.parse_irqs(" CPU0 CPU1\n 24: 100 20 PCI-MSI nvidia\n ERR: 5\n"),
             "error": None}
        b = {"rows": census.parse_irqs(" CPU0 CPU1\n 24: 130 25 PCI-MSI nvidia\n ERR: 7\n"),
             "error": None}
        result = census.irq_delta(a, b)
        self.assertEqual(result["rows"]["24"]["count"], 35)
        self.assertEqual(result["rows"]["ERR"]["count"], 2)
        self.assertIsNone(census.irq_delta(a, {"rows": {}, "error": "unreadable"})["rows"])

    def test_reassigned_irq_is_not_attributed_to_the_old_device(self):
        a = {"rows": census.parse_irqs(" 24: 100 PCI-MSI nvidia"), "error": None}
        b = {"rows": census.parse_irqs(" 24: 130 PCI-MSI other"), "error": None}
        result = census.irq_delta(a, b)
        self.assertEqual(result["changed_or_regressed"], ["24"])
        self.assertEqual(result["rows"], {})


if __name__ == "__main__":
    unittest.main()
