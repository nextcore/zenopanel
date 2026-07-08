import sqlite3

def check_db(path):
    print(f"=== Checking {path} ===")
    try:
        conn = sqlite3.connect(path)
        cursor = conn.cursor()
        cursor.execute("SELECT * FROM db_servers")
        rows = cursor.fetchall()
        for r in rows:
            print(r)
        conn.close()
    except Exception as e:
        print("Error:", e)

check_db("dist/zenopanel-v1.3.13/zeno.db")
check_db("zeno.db")
