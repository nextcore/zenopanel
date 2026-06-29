import sqlite3

conn = sqlite3.connect("dist/zenopanel-v1.0.9/zeno.db")
cursor = conn.cursor()

cursor.execute("SELECT * FROM db_servers")
rows = cursor.fetchall()
print("=== db_servers ===")
for r in rows:
    print(r)

conn.close()
