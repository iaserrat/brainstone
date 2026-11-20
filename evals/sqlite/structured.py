"""Exercise Brainstone's persisted schema contract, separately from comparisons."""
import argparse
import json
from pathlib import Path
import sys

from gateway import Gateway
from runner import Campaign

VALUE_SCHEMA = {"type":"object","properties":{"value":{"type":"string"}},"required":["value"],"additionalProperties":False}
LIVE = [
    ("exact", "Return a JSON object with value set to indigo.", VALUE_SCHEMA, {"value":"indigo"}),
    ("nested", 'Extract name Zoë and age 31 as {"person":{"name":...,"age":...}}.',
     {"type":"object","properties":{"person":{"type":"object","properties":{"name":{"type":"string"},"age":{"type":"integer"}},"required":["name","age"],"additionalProperties":False}},"required":["person"],"additionalProperties":False},
     {"person":{"name":"Zoë","age":31}}),
    ("enum", 'Return status approved and count 3 in a JSON object.',
     {"type":"object","properties":{"status":{"enum":["approved","rejected"],"type":"string"},"count":{"type":"integer"}},"required":["status","count"],"additionalProperties":False},
     {"status":"approved","count":3}),
]


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--live", action="store_true")
    parser.add_argument("--output", required=True)
    args = parser.parse_args()
    output = Path(args.output).resolve()
    if output.exists():
        parser.error("choose a fresh output directory")
    with Gateway(output, live=args.live, provider="wafer/us", limit="0.03") as gateway:
        campaign = Campaign(output, gateway, sys.executable, ["brainstone"])
        cases = LIVE if args.live else [
            ("valid", "Return JSON.", VALUE_SCHEMA, {"value":"ok"}),
            ("invalid", "Return JSON.", {**VALUE_SCHEMA,"properties":{"value":{"type":"string","enum":["allowed"]}}}, None),
        ]
        for name, text, schema, expected in cases:
            config = campaign.config("brainstone", "structured-"+name, text)
            config["output_schema"] = schema
            result = campaign.invoke("brainstone", config)
            terminal = next((e for e in reversed(result["events"]) if e.get("event")=="terminal"), {})
            if expected is None:
                passed = terminal.get("status")=="failed" and terminal.get("code")=="structured_output_invalid"
            else:
                try:
                    passed = terminal.get("status")=="completed" and json.loads(terminal.get("value",""))==expected
                except ValueError:
                    passed = False
            campaign.record("brainstone", "structured-"+name, passed, **result)
        if not all(row["passed"] for row in campaign.results):
            raise SystemExit(1)


if __name__ == "__main__":
    main()
