import requests
import re
import json
import time

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

print("=== Starting Bulk Data Test ===")

# Generate 500 rows for bulk insert
values = []
for i in range(1, 501):
    username = f"bulk_user_{i}"
    email = f"bulk_{i}@example.com"
    values.append(f"('{username}', '{email}')")

bulk_sql = f"INSERT IGNORE INTO users (username, email) VALUES {', '.join(values)}"

print(f"\n1. Inserting 500 rows in bulk...")
start_time = time.time()
insert_res = run_query(bulk_sql, is_select=False)
elapsed = time.time() - start_time
print(f"Bulk Insert Result: {json.dumps(insert_res, indent=2)}")
print(f"Time taken: {elapsed:.3f} seconds")

# 2. Get total row count
print("\n2. Getting total row count in users table...")
count_res = run_query("SELECT COUNT(*) as count FROM users", is_select=True)
print(f"Count Result: {json.dumps(count_res, indent=2)}")

# 3. Read first 5 bulk rows
print("\n3. Fetching first 5 bulk users...")
sample_res = run_query("SELECT id, username, email FROM users WHERE username LIKE 'bulk_user_%' LIMIT 5", is_select=True)
print(f"Sample Result: {json.dumps(sample_res, indent=2)}")
