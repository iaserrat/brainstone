"""Run real framework subprocesses; score native contracts without receipt shims."""
import argparse
from contextlib import closing
import json
from pathlib import Path
import sqlite3
import subprocess
import sys
import time

from budget import MODEL
from gateway import Gateway

ROOT = Path(__file__).resolve().parents[2]
SYSTEM = 'Follow the user task. Use tools only when requested. Return a JSON object with exactly one key, "value", whose value is a string.'


def normalize(value):
    for _ in range(3):
        if isinstance(value, str):
            try:
                decoded = json.loads(value)
            except (ValueError, TypeError):
                return value
            if isinstance(decoded, (dict, str)):
                value = decoded
                continue
        if isinstance(value, dict) and set(value) == {"value"}:
            value = value["value"]
            continue
        break
    return value


def counts(path):
    if not Path(path).exists():
        return {"effects":0, "attempts":0, "values":[]}
    with closing(sqlite3.connect(path)) as connection:
        tables = {row[0] for row in connection.execute("SELECT name FROM sqlite_master WHERE type='table'")}
        result = {name:connection.execute(f"SELECT count(*) FROM {name}").fetchone()[0] if name in tables else 0
                  for name in ("effects", "attempts")}
        result["values"] = [row[0] for row in connection.execute("SELECT value FROM effects ORDER BY retry_key")] if "effects" in tables else []
        return result



class Campaign:
    def __init__(self, output, gateway, python, frameworks):
        self.output, self.gateway = Path(output), gateway
        self.frameworks = frameworks
        self.commands = {
            "brainstone":[str(ROOT / "target/debug/examples/eval_sqlite")],
            "agent-rs":[str(ROOT / "evals/sqlite/agent-rs/target/debug/brainstone-agent-rs-eval")],
            "langgraph":[python, str(ROOT / "evals/sqlite/langgraph_agent.py")],
        }
        self.results = []

    def config(self, framework, case, text):
        directory = self.output / framework / case
        directory.mkdir(parents=True, exist_ok=True)
        return {"action":"invoke", "database":str(directory / "state.sqlite"),
                "effects_database":str(directory / "effects.sqlite"), "conversation":case,
                "request_id":"message-1", "system":SYSTEM, "text":text,
                "endpoint":self.gateway.endpoint(framework+"--"+case), "model":MODEL,
                "fault":None, "operation_id":None}

    def invoke(self, framework, config):
        path = Path(config["database"]).parent / (config["action"]+"-"+str(time.time_ns())+".json")
        path.write_text(json.dumps(config, ensure_ascii=False, indent=2))
        started, before = time.monotonic(), len(self.gateway.records)
        try:
            process = subprocess.run(self.commands[framework]+[str(path)], capture_output=True, text=True, timeout=120)
            events = [json.loads(line) for line in process.stdout.splitlines() if line.startswith("{")]
            result = {"exit_code":process.returncode, "events":events, "stderr":process.stderr[-2000:]}
        except subprocess.TimeoutExpired:
            result = {"exit_code":124, "events":[], "stderr":"driver_timeout"}
        result.update(seconds=time.monotonic()-started, requests=len(self.gateway.records)-before)
        path.with_suffix(".result.json").write_text(json.dumps(result, ensure_ascii=False, indent=2))
        return result

    def record(self, framework, case, passed, **details):
        result = {"framework":framework, "case":case, "passed":bool(passed), **details}
        self.results.append(result)
        (self.output / "results.json").write_text(json.dumps(self.results, ensure_ascii=False, indent=2))
        print(json.dumps(result, ensure_ascii=False), flush=True)

    def tasks(self, repetitions):
        tasks = json.loads((Path(__file__).parent / "tasks.json").read_text())
        # Rotate order per repetition to reduce fixed ordering effects.
        for repeat in range(repetitions):
            order = self.frameworks[repeat % len(self.frameworks):]+self.frameworks[:repeat % len(self.frameworks)]
            for task in tasks:
                for framework in order:
                    case = f"{task['name']}-{repeat}"
                    config = self.config(framework, case, task["text"])
                    result = self.invoke(framework, config)
                    terminal = next((e for e in reversed(result["events"]) if e.get("event")=="terminal"), {})
                    value = normalize(terminal.get("value"))
                    ledger = counts(config["effects_database"])
                    effect_contract = ledger["effects"]==task.get("effects",0) and ledger["attempts"]==task.get("effects",0)
                    if task.get("effects"):
                        effect_contract = effect_contract and ledger["values"]==[task["expected"]]
                    passed = terminal.get("status")=="completed" and value==task["expected"] and effect_contract
                    self.record(framework, case, passed, answer_correct=value==task["expected"],
                                effect_contract=effect_contract, value=value, ledger=ledger, **result)

    def history(self, repetitions):
        for repeat in range(repetitions):
            for framework in self.frameworks:
                config = self.config(framework, f"history-{repeat}",
                    'Remember the verification token cortex-☕. Return a JSON object with value set to stored.')
                first = self.invoke(framework, config)
                config.update(request_id="message-2", text='What exact verification token did I give you? Return it in a JSON object with key value.')
                second = self.invoke(framework, config)
                terminal = next((e for e in reversed(second["events"]) if e.get("event")=="terminal"), {})
                self.record(framework, f"history-{repeat}", terminal.get("status")=="completed" and normalize(terminal.get("value"))=="cortex-☕",
                            first=first, value=normalize(terminal.get("value")), **second)

    def concurrency(self):
        for framework in self.frameworks:
            config = self.config(framework, "concurrent", 'Call RecordValue once with value durable-☕. Return its value as JSON with key value.')
            config["fault"] = "after_accept"
            accepted = self.invoke(framework, config)
            receipt = next((e for e in accepted["events"] if e.get("event")=="accepted"), {})
            config.update(action="resume", fault=None, operation_id=receipt.get("operation_id") or config["conversation"])
            processes = []
            before = len(self.gateway.records)
            for index in range(2):
                path = Path(config["database"]).parent / f"concurrent-{index}.json"
                path.write_text(json.dumps(config, ensure_ascii=False))
                processes.append(subprocess.Popen(self.commands[framework]+[str(path)], stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True))
            outcomes = []
            for process in processes:
                try:
                    stdout, stderr = process.communicate(timeout=120)
                except subprocess.TimeoutExpired:
                    process.kill()
                    stdout, stderr = process.communicate()
                outcomes.append({"exit_code":process.returncode, "stdout":stdout, "stderr":stderr[-2000:]})
            config["action"] = "inspect"
            inspection = self.invoke(framework, config)
            terminal = next((e for e in inspection["events"] if e.get("event")=="terminal"), {})
            ledger = counts(config["effects_database"])
            self.record(framework, "concurrent_resume", terminal.get("status")=="completed" and ledger["effects"]==1,
                        contract="one application effect under concurrent drivers", ledger=ledger,
                        requests=len(self.gateway.records)-before, drivers=outcomes, inspection=inspection)

    def faults(self):
        text = 'Call RecordValue exactly once with value durable-☕. Return its value as JSON with key value.'
        for framework in self.frameworks:
            for fault in ["after_accept", "after_effect", "after_tool_result"]:
                config = self.config(framework, fault, text)
                config["fault"] = fault
                crashed = self.invoke(framework, config)
                accepted = next((e for e in crashed["events"] if e.get("event")=="accepted"), {})
                operation = accepted.get("operation_id")
                # If no explicit receipt, LangGraph's native thread identity is the operation locator.
                config.update(action="resume", fault=None, operation_id=operation or config["conversation"])
                if framework == "brainstone":
                    time.sleep(2.2)  # Adapter uses a two-second fenced lease.
                recovered = self.invoke(framework, config)
                terminal = next((e for e in reversed(recovered["events"]) if e.get("event")=="terminal"), {})
                ledger = counts(config["effects_database"])
                passed = crashed["exit_code"]==73 and terminal.get("status")=="completed" and normalize(terminal.get("value"))=="durable-☕" and ledger["effects"]==1
                if fault=="after_tool_result":
                    passed = passed and ledger["attempts"]==1
                self.record(framework, fault, passed, ledger=ledger, crashed=crashed, recovered=recovered)
                if fault=="after_tool_result" and terminal.get("status")=="completed":
                    self.retries(framework, config)

    def retries(self, framework, config):
        config.update(action="inspect")
        read = self.invoke(framework, config)
        last = read["events"][-1] if read["events"] else {}
        self.record(framework, "terminal_read", last.get("status")=="completed" and read["requests"]==0, **read)
        config.update(action="invoke")
        retry = self.invoke(framework, config)
        last = retry["events"][-1] if retry["events"] else {}
        self.record(framework, "same_input_retry", last.get("status")=="completed" and normalize(last.get("value"))=="durable-☕" and retry["requests"]==0,
                    contract="native request-id deduplication", **retry)
        config["text"] = "Return changed in a JSON value object."
        changed = self.invoke(framework, config)
        rejected = any(e.get("event")=="driver_error" and e.get("code")=="idempotency_conflict" for e in changed["events"])
        self.record(framework, "changed_input_retry", rejected and changed["requests"]==0,
                    contract="native conflicting retry rejection", **changed)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--live", action="store_true")
    parser.add_argument("--provider", help="Pin one OpenRouter endpoint tag for live comparisons")
    parser.add_argument("--output", default=str(ROOT / "data/evals/latest"))
    parser.add_argument("--python", default=sys.executable)
    parser.add_argument("--frameworks", nargs="+", default=["brainstone", "agent-rs", "langgraph"])
    parser.add_argument("--repetitions", type=int, default=2)
    args = parser.parse_args()
    if args.repetitions < 1 or args.repetitions > 3:
        parser.error("repetitions must be 1..3")
    output = Path(args.output).resolve()
    if (output / "results.json").exists():
        parser.error("choose a fresh output directory to avoid mixing runs")
    with Gateway(output, live=args.live, env_file=ROOT / ".env", provider=args.provider) as gateway:
        import hashlib
        adapter_paths = [ROOT / "target/debug/examples/eval_sqlite", ROOT / "evals/sqlite/agent-rs/target/debug/brainstone-agent-rs-eval", ROOT / "evals/sqlite/langgraph_agent.py"]
        fingerprints = {str(path.relative_to(ROOT)):hashlib.sha256(path.read_bytes()).hexdigest() for path in adapter_paths if path.exists()}
        metadata = {"adapter_sha256":fingerprints, "working_tree_dirty":subprocess.call(["git","diff","--quiet"],cwd=ROOT)!=0, "model":MODEL, "provider":args.provider, "live":args.live,
                    "repetitions":args.repetitions, "system":SYSTEM,
                    "base_commit":subprocess.check_output(["git","rev-parse","HEAD"],cwd=ROOT,text=True).strip(),
                    "gateway":"buffered upstream JSON converted to SSE; no streaming benchmark",
                    "output_tokens":768,"budget_usd":"0.50"}
        (output / "manifest.json").write_text(json.dumps(metadata,ensure_ascii=False,indent=2))
        campaign = Campaign(output, gateway, args.python, args.frameworks)
        if args.live:
            campaign.tasks(args.repetitions)
            campaign.history(args.repetitions)
        else:
            campaign.faults()
            campaign.concurrency()
        print(json.dumps({"budget":gateway.budget.snapshot()}), flush=True)


if __name__ == "__main__":
    main()
