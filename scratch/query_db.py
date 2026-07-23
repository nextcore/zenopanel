import sqlite3

conn = sqlite3.connect("dist/zenopanel-v1.7.12/zeno.db")
cursor = conn.cursor()

# Get db_servers
cursor.execute("SELECT id, name, driver, host, port, admin_user, admin_password, is_remote, pool_enabled, pool_port FROM db_servers")
print("db_servers:")
for row in cursor.fetchall():
    print(row)

# Get db_databases
cursor.execute("SELECT id, server_id, db_name, db_user, db_password FROM db_databases")
print("\ndb_databases:")
for row in cursor.fetchall():
    print(row)

conn.close()
