import requests
import re
import json
import time

session = requests.Session()
base_url = "http://127.0.0.1:8080"
print(f"Using base URL: {base_url}")

# 1. Get Login Page to obtain CSRF
res = session.get(f"{base_url}/login")
res.raise_for_status()

csrf_match = re.search(r"csrfToken = '([^']+)';", res.text)
if not csrf_match:
    csrf_match = re.search(r"const\s+csrfToken\s*=\s*'([^']+)';", res.text)

if not csrf_match:
    print("Failed to find CSRF token in page")
    exit(1)

csrf_token = csrf_match.group(1)
print(f"Extracted CSRF token: {csrf_token}")

# 2. Login
login_payload = {"username": "admin", "password": "admin"}
res = session.post(
    f"{base_url}/login",
    headers={"X-CSRF-Token": csrf_token},
    json=login_payload
)
print("Login status:", res.status_code)
print("Login cookies:", session.cookies.get_dict())

# 3. Install mysql:5.7
# We use port 3311 for mysql:5.7 and pool_port 6035 to prevent any port conflicts
db_name = "mysql-test-57"
db_port = 3311
pool_port = 6035

install_payload = {
    "engine": "mysql:5.7",
    "name": db_name,
    "port": db_port,
    "root_password": "zenopanel-mysql57-pass",
    "data_dir": f"/var/lib/zenopanel/db/{db_name}",
    "is_remote": 1,
    "pool_enabled": 0,  # We will toggle it later via API to test toggle-pool
    "pool_port": pool_port
}

print(f"Installing DB server '{db_name}' on port {db_port}...")
res = session.post(
    f"{base_url}/api/database/install-server",
    headers={"X-CSRF-Token": csrf_token},
    json=install_payload
)
print("Install status:", res.status_code)
print("Install response:", res.text)

# Wait a moment for container deployment and initialization
print("Waiting 15 seconds for DB container to start up...")
time.sleep(15)

# 4. Find the registered server ID
res = session.get(f"{base_url}/api/database/servers")
servers_json = res.json()
print("Registered servers list:", json.dumps(servers_json, indent=2))

server_id = None
servers_list = servers_json.get("data", []) if isinstance(servers_json, dict) else servers_json
for s in servers_list:
    if isinstance(s, dict) and s.get("name") == db_name:
        server_id = s.get("id")
        break

if not server_id:
    print(f"Failed to find server ID for {db_name}")
    exit(1)

print(f"Found Server ID: {server_id}")

# 5. Create a user database first (so that ProxySQL has users to mirror)
print("Creating a user database and user...")
create_db_payload = {
    "server_id": server_id,
    "db_name": "pool_test_db",
    "db_user": "pool_test_user",
    "db_password": "pool_test_password_123",
    "access_type": "remote", # Allows % access
    "charset": "utf8mb4",
    "collation": "utf8mb4_unicode_ci",
    "description": "Test DB for Connection Pool",
    "host_ip": "127.0.0.1",
    "host_port": db_port
}
res = session.post(
    f"{base_url}/api/database/create",
    headers={"X-CSRF-Token": csrf_token},
    json=create_db_payload
)
print("Create DB status:", res.status_code)
print("Create DB response:", res.text)

# 6. Enable Connection Pool (ProxySQL)
print(f"Enabling ProxySQL pool on port {pool_port}...")
toggle_payload = {
    "id": server_id,
    "pool_enabled": 1
}
res = session.post(
    f"{base_url}/api/database/servers/toggle-pool",
    headers={"X-CSRF-Token": csrf_token},
    json=toggle_payload
)
print("Toggle pool status:", res.status_code)
print("Toggle pool response:", res.text)

# Wait 5 seconds for ProxySQL container to start
print("Waiting 5 seconds for ProxySQL to start...")
time.sleep(5)

# 7. Test connection to connection pool using /api/database/query
# Since ProxySQL mirrors the users, we can run queries using user credentials
print("Testing direct raw query on the user database via API...")
query_payload = {
    "connection_name": "pool_test_db",  # Matches the created database name
    "sql": "SELECT 1 + 1 AS result",
    "is_select": True
}
res = session.post(
    f"{base_url}/api/database/query",
    headers={"X-CSRF-Token": csrf_token},
    json=query_payload
)
print("Query status:", res.status_code)
print("Query response:", res.text)

# 8. Check status of database servers
res = session.get(f"{base_url}/api/database/servers/status")
print("Servers status:", res.status_code)
print("Servers status response:", res.text)
