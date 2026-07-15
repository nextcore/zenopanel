import sqlite3
import pprint

conn = sqlite3.connect("zeno_internal.db")
cursor = conn.cursor()

# Get tables list
cursor.execute("SELECT name FROM sqlite_master WHERE type='table'")
print("Tables in zeno_internal.db:")
print(cursor.fetchall())

try:
    cursor.execute("PRAGMA table_info(waf_logs)")
    print("\nColumns of waf_logs:")
    pprint.pprint(cursor.fetchall())

    print("\nLast 10 WAF logs:")
    cursor.execute("SELECT * FROM waf_logs ORDER BY id DESC LIMIT 10")
    for row in cursor.fetchall():
        pprint.pprint(row)
except Exception as e:
    print("Error:", e)

conn.close()
