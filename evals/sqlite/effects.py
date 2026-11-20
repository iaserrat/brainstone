"""The same application-owned effect ledger for Python comparison adapters."""
import os
from contextlib import closing
import sqlite3


def record(config, retry_key, value):
    if not isinstance(value, str):
        raise ValueError("RecordValue requires a string")
    with closing(sqlite3.connect(config["effects_database"])) as database, database:
        database.execute("PRAGMA journal_mode=WAL")
        database.execute("PRAGMA synchronous=FULL")
        database.execute("CREATE TABLE IF NOT EXISTS effects(retry_key TEXT PRIMARY KEY,value TEXT NOT NULL)")
        database.execute("CREATE TABLE IF NOT EXISTS attempts(retry_key TEXT NOT NULL,value TEXT NOT NULL)")
        database.execute("INSERT INTO attempts VALUES(?,?)", (retry_key, value))
        database.execute("INSERT INTO effects VALUES(?,?) ON CONFLICT DO NOTHING", (retry_key, value))
        stored = database.execute("SELECT value FROM effects WHERE retry_key=?", (retry_key,)).fetchone()[0]
        if stored != value:
            raise ValueError("effect retry changed its input")
    if config.get("fault") == "after_effect":
        os._exit(73)
    return {"value": stored}
