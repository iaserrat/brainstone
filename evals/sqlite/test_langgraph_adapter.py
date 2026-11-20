"""The graph adapter must use persisted execution identity and bound inference."""
from contextlib import closing
import json
from pathlib import Path
import sqlite3
import tempfile
import unittest
from unittest.mock import patch

from langchain_core.messages import AIMessage, HumanMessage
from langgraph.checkpoint.sqlite import SqliteSaver

import langgraph_agent as adapter


class AdapterContract(unittest.TestCase):
    def config(self, database):
        return {"conversation": "thread", "effects_database": database,
                "fault": None, "system": "fixture", "model": "fixture"}

    def message(self, identity, value):
        return AIMessage(id=identity, content="", tool_calls=[{
            "name": "RecordValue", "id": "provider-reused-id", "args": {"value": value},
        }])

    def test_same_persisted_assistant_message_replays_one_effect(self):
        with tempfile.TemporaryDirectory() as directory:
            path = str(Path(directory) / "effects.sqlite")
            config = self.config(path)
            message = self.message("assistant-1", "one")
            with SqliteSaver.from_conn_string(str(Path(directory) / "graph.sqlite")) as saver:
                graph = adapter.build(config, saver)
                runnable = {"configurable": {"thread_id": "thread"}}
                graph.update_state(runnable, {"messages": [HumanMessage(content="Record one"), message]})
                saved = graph.get_state(runnable).values["messages"][-1]
            first = adapter.tool_result(config, saved, saved.tool_calls[0])
            with SqliteSaver.from_conn_string(str(Path(directory) / "graph.sqlite")) as saver:
                reopened = adapter.build(config, saver)
                restored = reopened.get_state(runnable).values["messages"][-1]
                second = adapter.tool_result(config, restored, restored.tool_calls[0])
            self.assertEqual(saved.id, restored.id)
            self.assertEqual(json.loads(first.content), json.loads(second.content))
            with closing(sqlite3.connect(path)) as database:
                self.assertEqual(database.execute("SELECT count(*) FROM effects").fetchone()[0], 1)
                self.assertEqual(database.execute("SELECT count(*) FROM attempts").fetchone()[0], 2)

    def test_distinct_assistant_messages_can_reuse_a_provider_call_id(self):
        with tempfile.TemporaryDirectory() as directory:
            path = str(Path(directory) / "effects.sqlite")
            config = self.config(path)
            for identity, value in [("assistant-1", "one"), ("assistant-2", "two")]:
                message = self.message(identity, value)
                result = adapter.tool_result(config, message, message.tool_calls[0])
                self.assertEqual(json.loads(result.content), {"value": value})
            with closing(sqlite3.connect(path)) as database:
                values = database.execute("SELECT value FROM effects ORDER BY value").fetchall()
                self.assertEqual(values, [("one",), ("two",)])

    def test_model_call_cap_survives_resume(self):
        calls = []

        def generate(config, messages):
            calls.append(len(calls))
            return self.message("assistant-" + str(len(calls)), "recorded")

        with tempfile.TemporaryDirectory() as directory:
            config = self.config(str(Path(directory) / "effects.sqlite"))
            with SqliteSaver.from_conn_string(str(Path(directory) / "graph.sqlite")) as saver:
                graph = adapter.build(config, saver)
                runnable = {"configurable": {"thread_id": "thread"}, "recursion_limit": 20}
                with patch.object(adapter, "generate", generate):
                    with self.assertRaisesRegex(adapter.DriverError, "model_call_limit"):
                        graph.invoke({"messages": [HumanMessage(content="Keep recording")], "model_calls": 0}, runnable, durability="sync")
                    self.assertEqual(len(calls), 4)
                    with self.assertRaisesRegex(adapter.DriverError, "model_call_limit"):
                        graph.invoke(None, runnable, durability="sync")
                    self.assertEqual(len(calls), 4)


if __name__ == "__main__":
    unittest.main()
