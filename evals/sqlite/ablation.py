"""Separate provider request formatting effects from framework execution behavior."""
import argparse
import json
from pathlib import Path
import urllib.request

from budget import MODEL
from gateway import Gateway
from runner import SYSTEM

TOOL = {"type":"function", "function":{"name":"RecordValue",
        "description":"Record a value in the application ledger and return it.",
        "parameters":{"type":"object","properties":{"value":{"type":"string"}},"required":["value"],"additionalProperties":False}}}
SCHEMA = {"type":"json_schema", "json_schema":{"name":"agent_output", "strict":False,
          "schema":{"type":"object","properties":{"value":{"type":"string"}},"required":["value"],"additionalProperties":False}}}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", required=True)
    parser.add_argument("--live", action="store_true", help="Explicitly enable paid model calls")
    args = parser.parse_args()
    if not args.live:
        parser.error("the request-format diagnostic requires --live")
    output = Path(args.output).resolve()
    if output.exists():
        parser.error("choose a fresh output directory")
    rows = []
    with Gateway(output, live=True, env_file=Path(__file__).resolve().parents[2] / ".env", provider="wafer/us", limit="0.03") as gateway:
        for repeat in range(2):
            for requested in (True, False):
                text = ('Call RecordValue exactly once with value durable-☕. Then return the recorded value in a JSON object with the key value.' if requested
                        else 'Return exactly a JSON object with value set to the string indigo.')
                for typed_wire in (False, True):
                    for schema in (False, True):
                        messages = [{"role":"developer" if typed_wire else "system","content":SYSTEM},
                                    {"role":"user","content":json.dumps(text) if typed_wire else text}]
                        body = {"model":MODEL,"messages":messages,"tools":[TOOL],"stream":False}
                        if schema:
                            body["response_format"] = SCHEMA
                        request = urllib.request.Request(gateway.endpoint("ablation")+"/chat/completions", data=json.dumps(body).encode(),headers={"Content-Type":"application/json"})
                        with urllib.request.urlopen(request,timeout=100) as response:
                            data = json.load(response)
                        message = data["choices"][0]["message"]
                        row = {"repeat":repeat,"requested":requested,"typed_wire":typed_wire,"schema":schema,
                               "called_tool":bool(message.get("tool_calls")),"message":message}
                        rows.append(row)
                        (output / "ablation.json").write_text(json.dumps(rows,ensure_ascii=False,indent=2))
                        print(json.dumps({k:v for k,v in row.items() if k!="message"}),flush=True)


if __name__ == "__main__":
    main()
