import sqlite3

db_path = "dist/zenopanel-v1.4.0/zeno.db"
conn = sqlite3.connect(db_path)
cursor = conn.cursor()

cursor.execute("SELECT id, name, driver, host, port, admin_user, admin_password, is_remote, pool_enabled, pool_port, engine, data_dir, mem_limit, cpus FROM db_servers")
print("Registered DB Servers:")
for row in cursor.fetchall():
    print(row)

conn.close()
