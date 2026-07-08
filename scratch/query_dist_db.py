import sqlite3

db_path = "dist/zenopanel-v1.4.0/zeno.db"
conn = sqlite3.connect(db_path)
cursor = conn.cursor()

# Get schema of db_servers
cursor.execute("PRAGMA table_info(db_servers)")
print("db_servers schema in dist:")
for row in cursor.fetchall():
    print(row)

# Get settings
cursor.execute("SELECT key, value FROM settings")
print("\nSettings in dist:")
for row in cursor.fetchall():
    print(row)

conn.close()
