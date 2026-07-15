import sqlite3
import pprint
import os

dbs = [
    "zeno.db",
    "dist/zeno.db",
    "dist/zenopanel-v1.5.16/zeno.db"
]

for db in dbs:
    if os.path.exists(db):
        print(f"\n--- Checking {db} ---")
        try:
            conn = sqlite3.connect(db)
            cursor = conn.cursor()
            cursor.execute("SELECT * FROM waf_logs WHERE reason LIKE '%SSRF%' ORDER BY id DESC LIMIT 5")
            rows = cursor.fetchall()
            print(f"Found {len(rows)} SSRF logs:")
            for row in rows:
                pprint.pprint(row)
            conn.close()
        except Exception as e:
            print("Error:", e)
