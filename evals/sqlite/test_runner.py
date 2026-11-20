"""Scoring must not mistake infrastructure failure for an idempotency contract."""
from pathlib import Path
import sqlite3
import tempfile
import unittest
from unittest.mock import Mock

from runner import Campaign, counts, normalize


class RunnerTests(unittest.TestCase):
    def test_generic_driver_failure_is_not_a_conflicting_retry_rejection(self):
        with tempfile.TemporaryDirectory() as directory:
            campaign = Campaign(directory, Mock(records=[]), "python3", ["brainstone"])
            success = {"events":[{"event":"terminal","status":"completed","value":"durable-☕"}],"requests":0}
            failure = {"events":[{"event":"driver_error","code":"adapter_error"}],"requests":0}
            campaign.invoke = Mock(side_effect=[success, success, failure])
            campaign.record = Mock()
            campaign.retries("brainstone", {})
            self.assertFalse(campaign.record.call_args_list[-1].args[2])

    def test_effect_evidence_includes_actual_values_and_attempts(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "effects.db"
            connection = sqlite3.connect(path)
            connection.execute("CREATE TABLE effects(retry_key TEXT PRIMARY KEY,value TEXT)")
            connection.execute("CREATE TABLE attempts(retry_key TEXT,value TEXT)")
            connection.execute("INSERT INTO effects VALUES('one','Zoë ☕')")
            connection.executemany("INSERT INTO attempts VALUES(?,?)", [("one","Zoë ☕")]*2)
            connection.commit()
            connection.close()
            self.assertEqual(counts(path), {"effects":1,"attempts":2,"values":["Zoë ☕"]})

    def test_fact_normalization_does_not_claim_schema_validation(self):
        self.assertEqual(normalize('not JSON'), 'not JSON')
        self.assertEqual(normalize('{"value":"19"}'), '19')
        self.assertEqual(normalize('19'), '19')


if __name__ == "__main__":
    unittest.main()
