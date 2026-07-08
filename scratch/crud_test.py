import requests
import re
import json

session = requests.Session()
base_url = "http://127.0.0.1:8080"

res = session.get(f"{base_url}/login")
csrf_match = re.search(r"csrfToken = '([^']+)';", res.text)
if not csrf_match:
    csrf_match = re.search(r"const\s+csrfToken\s*=\s*'([^']+)';", res.text)

csrf_token = csrf_match.group(1)

# Login
session.post(
    f"{base_url}/login",
    headers={"X-CSRF-Token": csrf_token},
    json={"username": "admin", "password": "admin"}
)

def run_query(sql, is_select=False):
    res = session.post(
        f"{base_url}/api/database/query?connection=test_db",
        headers={"X-CSRF-Token": csrf_token},
        json={"sql": sql, "is_select": is_select}
    )
    if res.status_code == 200:
        return res.json()
    else:
        raise Exception(f"Query failed with status {res.status_code}: {res.text}")

print("=== Starting CRUD Operations ===")

# 1. CREATE
print("\n[C] Creating new row...")
create_res = run_query(
    "INSERT INTO users (username, email) VALUES ('crud_user', 'crud@example.com')",
    is_select=False
)
print("Create Result:", json.dumps(create_res, indent=2))

# 2. READ
print("\n[R] Reading all rows...")
read_res = run_query(
    "SELECT id, username, email FROM users",
    is_select=True
)
print("Read Result:", json.dumps(read_res, indent=2))

# 3. UPDATE
print("\n[U] Updating row's email...")
update_res = run_query(
    "UPDATE users SET email = 'crud_updated@example.com' WHERE username = 'crud_user'",
    is_select=False
)
print("Update Result:", json.dumps(update_res, indent=2))

# 4. READ (Verify Update)
print("\n[R] Reading updated row...")
verify_update_res = run_query(
    "SELECT id, username, email FROM users WHERE username = 'crud_user'",
    is_select=True
)
print("Verify Update Result:", json.dumps(verify_update_res, indent=2))

# 5. DELETE
print("\n[D] Deleting row...")
delete_res = run_query(
    "DELETE FROM users WHERE username = 'crud_user'",
    is_select=False
)
print("Delete Result:", json.dumps(delete_res, indent=2))

# 6. READ (Verify Delete)
print("\n[R] Reading all rows (Final state)...")
final_read_res = run_query(
    "SELECT id, username, email FROM users",
    is_select=True
)
print("Final Read Result:", json.dumps(final_read_res, indent=2))
