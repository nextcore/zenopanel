import sqlite3

conn = sqlite3.connect("zeno.db")
cursor = conn.cursor()

# Get users
cursor.execute("SELECT id, username, role, password_hash FROM users")
print("Users:")
for row in cursor.fetchall():
    print(row)

conn.close()
