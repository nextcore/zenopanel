import sqlite3

conn = sqlite3.connect("dist/zenopanel-v1.0.9/zeno.db")
cursor = conn.cursor()

# Get table info
cursor.execute("PRAGMA table_info(db_servers)")
columns = cursor.fetchall()
print("=== db_servers Schema ===")
for c in columns:
    print(c)

conn.close()
