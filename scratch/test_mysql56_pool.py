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

# 3. Clean up existing mysql-test-56 if any
res = session.get(f"{base_url}/api/database/servers")
servers_json = res.json()
servers_list = servers_json.get("data", []) if isinstance(servers_json, dict) else servers_json
for s in servers_list:
    if isinstance(s, dict) and s.get("name") == "mysql-test-56":
        print(f"Deleting existing server ID {s.get('id')}...")
        session.post(
            f"{base_url}/api/database/servers/delete",
            headers={"X-CSRF-Token": csrf_token},
            json={"id": s.get("id")}
        )

# Wait 5 seconds for cleanup
time.sleep(5)

# 4. Install mysql:5.6
db_name = "mysql-test-56"
db_port = 3312
pool_port = 6036

install_payload = {
    "engine": "mysql:5.6",
    "name": db_name,
    "port": db_port,
    "root_password": "zenopanel-mysql56-pass",
    "data_dir": f"/var/lib/zenopanel/db/{db_name}",
    "is_remote": 1,
    "pool_enabled": 0,
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
print("Waiting 20 seconds for DB container to start up...")
time.sleep(20)

# 5. Find the registered server ID
res = session.get(f"{base_url}/api/database/servers")
servers_json = res.json()

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

# 6. Create a user database first with username no longer than 16 chars for MySQL 5.6
print("Creating a user database and user...")
create_db_payload = {
    "server_id": server_id,
    "db_name": "pool_test_db",
    "db_user": "pool_test_56", # 12 characters, works on MySQL 5.6
    "db_password": "pool_test_password_123",
    "access_type": "remote",
    "charset": "utf8",
    "collation": "utf8_general_ci",
    "description": "Test DB for Connection Pool (MySQL 5.6)",
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

# 7. Enable Connection Pool (ProxySQL)
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

# Wait 10 seconds for ProxySQL container to start
print("Waiting 10 seconds for ProxySQL to start...")
time.sleep(10)

# Check status of database servers
res = session.get(f"{base_url}/api/database/servers/status")
print("Servers status:", res.status_code)
print("Servers status response:", res.text)

# 8. Disable Connection Pool (ProxySQL)
print("Disabling ProxySQL pool...")
toggle_payload = {
    "id": server_id,
    "pool_enabled": 0
}
res = session.post(
    f"{base_url}/api/database/servers/toggle-pool",
    headers={"X-CSRF-Token": csrf_token},
    json=toggle_payload
)
print("Disable pool status:", res.status_code)
print("Disable pool response:", res.text)

# Wait 5 seconds for container to stop and delete
time.sleep(5)

# Verify if container is gone
print("Verifying if pool container has been removed...")
res = session.get(f"{base_url}/api/database/servers/status")
print("Final Servers status:", res.status_code)
print("Final Servers status response:", res.text)
