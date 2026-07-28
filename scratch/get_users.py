import sqlite3
conn = sqlite3.connect('zeno.db')
cursor = conn.cursor()
cursor.execute("SELECT name FROM sqlite_master WHERE type='table';")
print("Tables:", cursor.fetchall())
try:
    cursor.execute("SELECT * FROM users;")
    print("Users:", cursor.fetchall())
except Exception as e:
    print("Error:", e)
conn.close()
