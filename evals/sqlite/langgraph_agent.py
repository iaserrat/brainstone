"""Native LangGraph SQLite adapter; no application receipt or outcome cache.

An accepted event identifies a synchronous input checkpoint in a thread. It is
not a per-request receipt. Resume supplies no new input; invoking again supplies
a new graph input even when add_messages recognizes the same message ID.
"""
from __future__ import annotations

import json
import os
from pathlib import Path
import sys
from typing import Annotated, TypedDict
import urllib.error
import urllib.request

from langchain_core.messages import AIMessage, BaseMessage, HumanMessage, ToolMessage
from langgraph.checkpoint.sqlite import SqliteSaver
from langgraph.graph import END, START, StateGraph
from langgraph.graph.message import add_messages

import effects

CRASH_EXIT_CODE = 73
MAX_MODEL_CALLS = 4
TOOL_NAME = "RecordValue"
TOOL = {
    "type": "function",
    "function": {
        "name": TOOL_NAME,
        "description": "Record a value in the application ledger and return it.",
        "parameters": {
            "type": "object",
            "properties": {"value": {"type": "string"}},
            "required": ["value"],
            "additionalProperties": False,
        },
    },
}


class State(TypedDict):
    messages: Annotated[list[BaseMessage], add_messages]
    model_calls: int


class DriverError(Exception):
    def __init__(self, code: str):
        self.code = code
        super().__init__(code)


def emit(event: str, **fields) -> None:
    print(json.dumps({"event": event, **fields}, ensure_ascii=False), flush=True)


def wire(message: BaseMessage) -> dict:
    if isinstance(message, HumanMessage):
        return {"role": "user", "content": message.content}
    if isinstance(message, ToolMessage):
        return {"role": "tool", "tool_call_id": message.tool_call_id, "content": message.content}
    if isinstance(message, AIMessage):
        result = {"role": "assistant", "content": message.content or None, **message.additional_kwargs}
        if message.tool_calls:
            result["tool_calls"] = [
                {"id": call["id"], "type": "function", "function": {
                    "name": call["name"], "arguments": json.dumps(call["args"]),
                }}
                for call in message.tool_calls
            ]
        return result
    raise DriverError("unsupported_message")


def generate(config: dict, messages: list[BaseMessage]) -> AIMessage:
    body = {
        "model": config["model"],
        "messages": [{"role": "system", "content": config["system"]}, *map(wire, messages)],
        "tools": [TOOL],
        "stream": False,
        "max_tokens": 768,
    }
    request = urllib.request.Request(
        config["endpoint"].rstrip("/") + "/chat/completions",
        data=json.dumps(body).encode(),
        headers={"Content-Type": "application/json", "Authorization": "Bearer eval-proxy"},
    )
    try:
        with urllib.request.urlopen(request, timeout=120) as response:
            result = json.load(response)
    except (urllib.error.URLError, TimeoutError) as error:
        raise DriverError("provider_transport_error") from error
    try:
        choice = result["choices"][0]
        message = choice["message"]
        if choice.get("finish_reason") not in ("stop", "tool_calls"):
            raise DriverError("incomplete_model_response")
        calls = [{"id": call["id"], "name": call["function"]["name"],
                  "args": json.loads(call["function"]["arguments"])}
                 for call in message.get("tool_calls", [])]
        opaque = {key: value for key, value in message.items()
                  if key not in {"role", "content", "tool_calls"}}
        return AIMessage(content=message.get("content") or "", tool_calls=calls, additional_kwargs=opaque)
    except (KeyError, IndexError, TypeError, ValueError) as error:
        raise DriverError("provider_response_invalid") from error


def tool_result(config: dict, message: AIMessage, call: dict) -> ToolMessage:
    args = call["args"]
    if call["name"] != TOOL_NAME:
        value = {"outcome": "failure", "code": "unknown_tool"}
    elif not isinstance(args, dict) or set(args) != {"value"} or not isinstance(args["value"], str):
        value = {"outcome": "failure", "code": "invalid_arguments"}
    else:
        if not message.id:
            raise DriverError("tool_identity_missing")
        # Provider call IDs can repeat in another turn. add_messages assigns the
        # assistant ID before its checkpoint; replay retains that native ID.
        retry_key = json.dumps([config["conversation"], message.id, call["id"]], separators=(",", ":"))
        value = effects.record(config, retry_key, args["value"])
    return ToolMessage(content=json.dumps(value), tool_call_id=call["id"])


def build(config: dict, saver: SqliteSaver):
    def model(state: State) -> State:
        count = state.get("model_calls", 0)
        if count >= MAX_MODEL_CALLS:
            raise DriverError("model_call_limit")
        return {"messages": [generate(config, state["messages"])], "model_calls": count + 1}

    def tools(state: State) -> State:
        message = state["messages"][-1]
        return {"messages": [tool_result(config, message, call) for call in message.tool_calls]}

    def route(state: State) -> str:
        return "tools" if state["messages"][-1].tool_calls else END

    graph = StateGraph(State)
    graph.add_node("model", model)
    graph.add_node("tools", tools)
    graph.add_edge(START, "model")
    graph.add_conditional_edges("model", route, ["tools", END])
    graph.add_edge("tools", "model")
    return graph.compile(checkpointer=saver)


def show_state(graph, runnable: dict, operation_id: str) -> None:
    snapshot = graph.get_state(runnable)
    if not snapshot.values:
        raise DriverError("operation_not_found")
    if snapshot.next:
        emit("pending", operation_id=operation_id, next=list(snapshot.next))
        return
    messages = snapshot.values.get("messages", [])
    if not messages or not isinstance(messages[-1], AIMessage) or messages[-1].tool_calls:
        raise DriverError("terminal_state_invalid")
    emit("terminal", status="completed", operation_id=operation_id, value=messages[-1].content)


def drive(config: dict, graph, runnable: dict) -> None:
    operation_id = config["conversation"]
    if config["action"] == "inspect":
        show_state(graph, runnable, operation_id)
        return
    if config["action"] == "invoke":
        message = HumanMessage(content=config["text"], id=config["request_id"])
        graph.invoke({"messages": [message], "model_calls": 0}, runnable, interrupt_before=["model"], durability="sync")
        emit("accepted", operation_id=operation_id, boundary="input_checkpoint")
        if config.get("fault") == "after_accept":
            os._exit(CRASH_EXIT_CODE)
    elif config["action"] != "resume":
        raise DriverError("invalid_action")
    snapshot = graph.get_state(runnable)
    if not snapshot.values:
        raise DriverError("operation_not_found")
    if snapshot.next:
        interrupt_after = ["tools"] if config.get("fault") == "after_tool_result" else None
        graph.invoke(None, runnable, interrupt_after=interrupt_after, durability="sync")
        if interrupt_after and graph.get_state(runnable).next:
            os._exit(CRASH_EXIT_CODE)
    show_state(graph, runnable, operation_id)


def main() -> None:
    config = json.loads(Path(sys.argv[1]).read_text())
    Path(config["database"]).parent.mkdir(parents=True, exist_ok=True)
    with SqliteSaver.from_conn_string(config["database"]) as saver:
        saver.conn.execute("PRAGMA journal_mode=WAL")
        saver.conn.execute("PRAGMA synchronous=FULL")
        saver.setup()
        graph = build(config, saver)
        runnable = {"configurable": {"thread_id": config["conversation"]}, "recursion_limit": MAX_MODEL_CALLS * 2 + 2}
        drive(config, graph, runnable)


if __name__ == "__main__":
    try:
        main()
    except DriverError as error:
        emit("driver_error", code=error.code)
        raise SystemExit(1) from None
    except Exception as error:
        emit("driver_error", code="adapter_error", detail=type(error).__name__)
        raise SystemExit(1) from None
