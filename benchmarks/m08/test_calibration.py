#!/usr/bin/env python3
"""Deterministic tests for the M08 calibration and labelling machinery.

Standard library `unittest`; no dependency is added for testing. Run directly
or through `make gates`, which invokes this file.
"""

from __future__ import annotations

import json
import os
import sys
import tempfile
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)

import calibrate as C  # noqa: E402
import label_edges as L  # noqa: E402


def rec(conf, label, kind="http-call", prov="route-matcher", at="a.py:1", edge=0):
    return {"confidence": conf, "label": label, "kind": kind,
            "provenance": prov, "at": at, "edge": edge}


class WilsonInterval(unittest.TestCase):
    def test_a_perfect_sample_does_not_claim_certainty(self):
        low, high = C.wilson(10, 10)
        self.assertLess(low, 1.0, "10/10 must not produce a lower bound of 1.0")
        self.assertEqual(high, 1.0)

    def test_a_wider_interval_for_a_smaller_sample(self):
        narrow = C.wilson(900, 1000)
        wide = C.wilson(9, 10)
        self.assertLess(wide[0], narrow[0])

    def test_an_empty_sample_has_no_interval(self):
        self.assertEqual(C.wilson(0, 0), (None, None))


class Summarise(unittest.TestCase):
    def test_unverifiable_records_are_excluded_from_accuracy(self):
        s = C.summarise([rec(0.9, "TRUE_POSITIVE"), rec(0.9, "FALSE_POSITIVE"),
                         rec(0.9, "UNVERIFIABLE")])
        self.assertEqual(s["produced"], 3)
        self.assertEqual(s["verified"], 2)
        self.assertEqual(s["observed_accuracy"], 0.5)
        self.assertEqual(s["unverifiable"], 1)

    def test_a_small_sample_is_flagged(self):
        self.assertFalse(C.summarise([rec(0.9, "TRUE_POSITIVE")])["sample_adequate"])
        many = [rec(0.9, "TRUE_POSITIVE") for _ in range(C.WEAK_SAMPLE)]
        self.assertTrue(C.summarise(many)["sample_adequate"])

    def test_a_group_with_no_verified_records_reports_no_accuracy(self):
        s = C.summarise([rec(0.7, "UNVERIFIABLE") for _ in range(50)])
        self.assertIsNone(s["observed_accuracy"])
        self.assertIsNone(s["ci95"])


class CalibrationError(unittest.TestCase):
    def test_perfect_calibration_is_zero(self):
        # 80 correct of 100 at stated 0.8.
        records = [rec(0.8, "TRUE_POSITIVE") for _ in range(80)]
        records += [rec(0.8, "FALSE_POSITIVE") for _ in range(20)]
        self.assertEqual(C.calibration_error(records)["ece"], 0.0)

    def test_underconfidence_is_measured_not_ignored(self):
        # Always right, but only claims 0.8: a real gap of 0.2.
        records = [rec(0.8, "TRUE_POSITIVE") for _ in range(100)]
        out = C.calibration_error(records)
        self.assertAlmostEqual(out["ece"], 0.2, places=6)
        self.assertAlmostEqual(out["mce"], 0.2, places=6)

    def test_ece_is_weighted_by_population(self):
        # 90 well-calibrated observations and 10 badly calibrated ones.
        records = [rec(0.9, "TRUE_POSITIVE") for _ in range(81)]
        records += [rec(0.9, "FALSE_POSITIVE") for _ in range(9)]   # 0.9 observed
        records += [rec(0.2, "TRUE_POSITIVE") for _ in range(10)]   # 1.0 observed, gap 0.8
        out = C.calibration_error(records)
        self.assertAlmostEqual(out["ece"], 0.08, places=6)
        self.assertAlmostEqual(out["mce"], 0.8, places=6)

    def test_unverifiable_share_is_reported(self):
        records = [rec(0.8, "TRUE_POSITIVE")] + [rec(0.8, "UNVERIFIABLE") for _ in range(3)]
        out = C.calibration_error(records)
        self.assertEqual(out["verified"], 1)
        self.assertEqual(out["excluded_unverifiable"], 3)
        self.assertEqual(out["excluded_share"], 0.75)


class ReliabilityBins(unittest.TestCase):
    def test_every_bin_is_present_even_when_empty(self):
        bins = C.reliability_bins([rec(0.85, "TRUE_POSITIVE")])
        self.assertEqual(len(bins), 10)
        self.assertEqual(bins["0.8-0.9"]["produced"], 1)
        self.assertEqual(bins["0.0-0.1"]["produced"], 0,
                         "an empty bin must be shown as empty, not omitted")

    def test_a_confidence_of_one_lands_in_the_top_bin(self):
        bins = C.reliability_bins([rec(1.0, "TRUE_POSITIVE")])
        self.assertEqual(bins["0.9-1.0"]["produced"], 1)


class Thresholds(unittest.TestCase):
    def test_raising_the_threshold_trades_coverage_for_precision(self):
        records = [rec(0.98, "TRUE_POSITIVE") for _ in range(10)]
        records += [rec(0.6, "FALSE_POSITIVE") for _ in range(10)]
        rows = {r["threshold"]: r for r in C.thresholds(records, [0.0, 0.9])}
        self.assertEqual(rows[0.0]["precision"], 0.5)
        self.assertEqual(rows[0.9]["precision"], 1.0)
        self.assertEqual(rows[0.9]["coverage_of_verified"], 0.5)
        self.assertEqual(rows[0.9]["false_positives_admitted"], 0)


class PathAgreement(unittest.TestCase):
    def test_a_declared_parameter_accepts_a_concrete_value(self):
        self.assertIs(L.paths_agree("/admin/cost-overrides/gpt-4o",
                                    "/admin/cost-overrides/{model}"), True)

    def test_a_static_segment_must_match(self):
        self.assertIs(L.paths_agree("/a/b", "/a/c"), False)

    def test_segment_counts_must_match(self):
        self.assertIs(L.paths_agree("/a", "/a/b"), False)

    def test_an_unknown_client_value_against_a_literal_is_not_decided(self):
        self.assertEqual(L.paths_agree("/user/projects/${id}", "/user/projects/create"),
                         L.UNKNOWN_AGAINST_LITERAL)

    def test_a_query_string_is_not_part_of_the_path(self):
        self.assertIs(L.paths_agree("/v1/customers/search?query=x", "/v1/{kind}/search"), True)

    def test_an_absolute_url_is_reduced_to_its_path(self):
        self.assertEqual(L.request_path("https://example.com/api/orders"), "/api/orders")


class HoldoutSplit(unittest.TestCase):
    def test_the_split_is_deterministic_and_ignores_the_label(self):
        sys.path.insert(0, HERE)
        import build_dataset as B
        base = {"repository": "r", "kind": "http-call", "at": "a.ts:1", "edge": 7}
        first = B.holdout_bucket({**base, "label": "TRUE_POSITIVE"}, "salt")
        second = B.holdout_bucket({**base, "label": "FALSE_POSITIVE"}, "salt")
        self.assertEqual(first, second, "the split must not be able to see the outcome")
        self.assertEqual(first, B.holdout_bucket(base, "salt"), "and must be deterministic")

    def test_both_buckets_are_produced(self):
        import build_dataset as B
        buckets = {B.holdout_bucket(
            {"repository": "r", "kind": "k", "at": f"f:{i}", "edge": i}, "salt")
            for i in range(200)}
        self.assertEqual(buckets, {"development", "holdout"})


class CorruptedArtifacts(unittest.TestCase):
    def test_a_dataset_without_records_yields_no_metrics(self):
        out = C.calibration_error([])
        self.assertIsNone(out["ece"])
        self.assertEqual(out["verified"], 0)

    def test_summarise_handles_an_empty_group(self):
        s = C.summarise([])
        self.assertEqual(s["produced"], 0)
        self.assertIsNone(s["observed_accuracy"])


if __name__ == "__main__":
    unittest.main(verbosity=2)
