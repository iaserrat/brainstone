"""Paid requests cannot inherit caller-controlled extra completions or services."""
import io
import json
import tempfile
import unittest
from unittest.mock import patch

from budget import MODEL, OUTPUT_TOKENS
from gateway import Gateway, sse


class GatewayTests(unittest.TestCase):
    def test_upstream_owns_model_spend_controls_and_rejects_paid_extras(self):
        with tempfile.TemporaryDirectory() as directory, patch("http.server.ThreadingHTTPServer"):
            gateway = Gateway(directory, provider="wafer/us")
            gateway.key = "fixture-key"
            body = {"model":MODEL,"messages":[{"role":"user","content":"test"}],
                    "n":99,"max_tokens":99999,"models":["expensive"],
                    "plugins":[{"id":"web"}],"provider":{"only":["expensive"]}}
            with patch("urllib.request.urlopen", return_value=io.BytesIO(b'{"choices":[]}')) as fetch:
                gateway.upstream(body)
            request = json.loads(fetch.call_args.args[0].data)
            self.assertEqual(request["n"], 1)
            self.assertEqual(request["max_tokens"], OUTPUT_TOKENS)
            self.assertFalse(request["stream"])
            self.assertEqual(request["provider"]["only"], ["wafer/us"])
            self.assertFalse(request["provider"]["allow_fallbacks"])
            self.assertNotIn("models", request)
            self.assertNotIn("plugins", request)
            gateway.server.server_close()

    def test_buffered_sse_preserves_tool_identity_and_final_usage(self):
        response = {"choices":[{"message":{"role":"assistant","content":None,
                    "tool_calls":[{"id":"call-1","function":{"name":"RecordValue","arguments":"{}"}}]},
                    "finish_reason":"tool_calls"}], "usage":{"prompt_tokens":10,"completion_tokens":4,"total_tokens":14,"cost":0.001}}
        frames = sse(response).decode().split("\n\n")
        first = json.loads(frames[0].removeprefix("data: "))
        last = json.loads(frames[1].removeprefix("data: "))
        self.assertEqual(first["choices"][0]["delta"]["tool_calls"][0]["id"], "call-1")
        self.assertEqual(last["usage"], response["usage"])
        self.assertEqual(frames[2], "data: [DONE]")


if __name__ == "__main__":
    unittest.main()
