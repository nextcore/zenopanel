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

# 1. Create database & user on server_id = 1 (mysql-test-57)
print("1. Creating database and database user...")
db_payload = {
    "server_id": 1,
    "db_name": "test_db",
    "db_user": "test_user",
    "db_password": "test_password_123",
    "access_type": "external",
    "description": "User test database",
    "charset": "utf8mb4",
    "collation": "utf8mb4_unicode_ci"
}
res = session.post(
    f"{base_url}/api/database/create",
    headers={"X-CSRF-Token": csrf_token},
    json=db_payload
)
print("Create DB status:", res.status_code)
print(res.text)

# 2. Create users table inside test_db
print("\n2. Creating users table inside test_db...")
create_table_sql = """
CREATE TABLE IF NOT EXISTS users (
    id INT AUTO_INCREMENT PRIMARY KEY,
    username VARCHAR(50) NOT NULL UNIQUE,
    email VARCHAR(100) NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
"""
query_payload = {
    "sql": create_table_sql,
    "is_select": False
}
res = session.post(
    f"{base_url}/api/database/query?connection=test_db",
    headers={"X-CSRF-Token": csrf_token},
    json=query_payload
)
print("Create table status:", res.status_code)
print(res.text)

# 3. Insert a sample user row
print("\n3. Inserting a sample user row...")
insert_sql = "INSERT INTO users (username, email) VALUES ('max_user', 'max@example.com');"
res = session.post(
    f"{base_url}/api/database/query?connection=test_db",
    headers={"X-CSRF-Token": csrf_token},
    json={
        "sql": insert_sql,
        "is_select": False
    }
)
print("Insert row status:", res.status_code)
print(res.text)

# 4. Select from users table
print("\n4. Querying users table...")
select_sql = "SELECT id, username, email, created_at FROM users;"
res = session.post(
    f"{base_url}/api/database/query?connection=test_db",
    headers={"X-CSRF-Token": csrf_token},
    json={
        "sql": select_sql,
        "is_select": True
    }
)
print("Select query status:", res.status_code)
try:
    print(json.dumps(res.json(), indent=2))
except Exception:
    print(res.text)
