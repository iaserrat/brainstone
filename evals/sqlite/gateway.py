"""Loopback-only, budgeted Chat Completions gateway; never logs credentials.

Live upstream responses are buffered and encoded as SSE for adapter parity.
These evaluations do not measure upstream streaming behavior or TTFT.
"""
import hashlib
import http.server
import json
from pathlib import Path
import threading
import time
import urllib.error
import urllib.request

from budget import Budget, INPUT_PRICE, MAX_BYTES, MODEL, OUTPUT_PRICE, OUTPUT_TOKENS


def key_from_env(path):
    for line in Path(path).read_text().splitlines():
        name, separator, value = line.strip().removeprefix("export ").partition("=")
        if separator and name.strip() == "OPENROUTER_API_KEY":
            return value.strip().strip("\"'")
    raise ValueError("OPENROUTER_API_KEY is missing")


def sse(response):
    choice = response["choices"][0]
    message = dict(choice["message"])
    for index, call in enumerate(message.get("tool_calls") or []):
        call["index"] = index
    base = {"id": response.get("id", "eval"), "model": response.get("model", MODEL)}
    first = dict(base, choices=[{"index": 0, "delta": message, "finish_reason": None}])
    last = dict(base, choices=[{"index": 0, "delta": {}, "finish_reason": choice["finish_reason"]}],
                usage=response.get("usage", {"prompt_tokens": 0, "completion_tokens": 0, "total_tokens": 0}))
    return ("data: " + json.dumps(first) + "\n\ndata: " + json.dumps(last) + "\n\ndata: [DONE]\n\n").encode()


class Gateway:
    def __init__(self, output, live=False, env_file=".env", limit="0.50", provider=None):
        self.output = Path(output)
        self.output.mkdir(parents=True, exist_ok=True)
        self.live = live
        self.provider = provider
        self.key = key_from_env(env_file) if live else None
        self.budget = Budget(limit)
        self.records = []
        self.lock = threading.Lock()
        gateway = self
        class Handler(http.server.BaseHTTPRequestHandler):
            def log_message(self, *_):
                pass
            def do_POST(self):
                gateway.handle(self)
        self.server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        self.thread = threading.Thread(target=self.server.serve_forever, daemon=True)

    def __enter__(self):
        self.thread.start()
        return self

    def __exit__(self, *_):
        self.server.shutdown()
        self.server.server_close()
        self.thread.join()
        (self.output / "budget.json").write_text(json.dumps(self.budget.snapshot(), indent=2))

    def endpoint(self, label):
        return f"http://127.0.0.1:{self.server.server_port}/{label}/v1"

    def handle(self, handler):
        size = int(handler.headers.get("Content-Length", "0"))
        if not 0 < size <= MAX_BYTES:
            return self.reject(handler, "eval_request_too_large")
        body = json.loads(handler.rfile.read(size))
        if body.get("model") != MODEL:
            return self.reject(handler, "eval_wrong_model")
        ticket, started = None, time.monotonic()
        try:
            if self.live:
                ticket = self.budget.reserve(size)
                response = self.upstream(body)
                self.budget.settle(ticket, response.get("usage", {}).get("cost"))
                ticket = None
            else:
                response = scripted(body)
            record = {"label": handler.path.split("/")[1], "request_sha256": hashlib.sha256(json.dumps(body, sort_keys=True).encode()).hexdigest(),
                      "seconds": time.monotonic()-started, "generation_id": response.get("id"),
                      "usage": response.get("usage"), "provider": response.get("provider"), "model": response.get("model")}
            self.log(record)
            raw = sse(response) if body.get("stream") else json.dumps(response).encode()
            handler.send_response(200)
            handler.send_header("Content-Type", "text/event-stream" if body.get("stream") else "application/json")
            handler.send_header("Content-Length", str(len(raw)))
            handler.end_headers()
            handler.wfile.write(raw)
        except Exception as error:
            if ticket is not None:
                self.budget.settle(ticket, None)
            self.log({"label":handler.path.split("/")[1], "error_type":type(error).__name__, "seconds":time.monotonic()-started})
            self.reject(handler, "eval_gateway_failed")

    def log(self, record):
        with self.lock:
            self.records.append(record)
            with (self.output / "requests.jsonl").open("a") as target:
                target.write(json.dumps(record)+"\n")

    def upstream(self, body):
        allowed = {"model", "messages", "tools", "tool_choice", "response_format", "parallel_tool_calls"}
        request = dict({name: value for name, value in body.items() if name in allowed},
                       stream=False, n=1, max_tokens=OUTPUT_TOKENS, temperature=0,
                       reasoning={"enabled":False},
                       provider={"sort":"price", "max_price":{"prompt":float(INPUT_PRICE), "completion":float(OUTPUT_PRICE)}})
        if self.provider:
            request["provider"].update(only=[self.provider], allow_fallbacks=False)
        request.pop("stream_options", None)
        request.pop("max_completion_tokens", None)
        raw = json.dumps(request).encode()
        connection = urllib.request.Request("https://openrouter.ai/api/v1/chat/completions", data=raw,
                     headers={"Authorization":"Bearer "+self.key, "Content-Type":"application/json"})
        with urllib.request.urlopen(connection, timeout=90) as response:
            return json.load(response)

    @staticmethod
    def reject(handler, code):
        raw = json.dumps({"error":{"code":code,"message":code}}).encode()
        handler.send_response(400)
        handler.send_header("Content-Type", "application/json")
        handler.send_header("Content-Length", str(len(raw)))
        handler.end_headers()
        handler.wfile.write(raw)


def scripted(body):
    time.sleep(0.05)  # Allow concurrent drivers to overlap in local fault tests.
    messages = body["messages"]
    last = messages[-1]
    if last.get("role") == "tool":
        content = json.loads(last["content"])
        value = content.get("value", content)
        message = {"role":"assistant", "content":json.dumps({"value":value}, ensure_ascii=False)}
        finish = "stop"
    elif "RecordValue" in str(last.get("content")):
        message = {"role":"assistant", "content":None, "tool_calls":[{"id":"eval-call-1", "type":"function",
                   "function":{"name":"RecordValue", "arguments":json.dumps({"value":"durable-☕"}, ensure_ascii=False)}}]}
        finish = "tool_calls"
    else:
        message = {"role":"assistant", "content":json.dumps({"value":"ok"})}
        finish = "stop"
    return {"id":"scripted", "model":MODEL, "choices":[{"index":0, "message":message, "finish_reason":finish}],
            "usage":{"prompt_tokens":100,"completion_tokens":20,"total_tokens":120,"cost":0}}
