import sqlite3

db_path = "dist/zenopanel-v1.4.0/zeno.db"
conn = sqlite3.connect(db_path)
cursor = conn.cursor()

# Get users
cursor.execute("SELECT id, username, role, password_hash FROM users")
print("Users in dist:")
for row in cursor.fetchall():
    print(row)

conn.close()
