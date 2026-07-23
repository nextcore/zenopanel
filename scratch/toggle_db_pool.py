import requests
import re
import json

session = requests.Session()

# 1. Login
res = session.get("http://127.0.0.1:8080/login")
csrf_token = re.search(r"csrfToken = '([^']+)';", res.text).group(1)

login_res = session.post(
    "http://127.0.0.1:8080/login",
    headers={"X-CSRF-Token": csrf_token},
    json={"username": "admin", "password": "admin"}
)
print("Login status:", login_res.status_code)

# 2. Get registered servers to find the ID of mysql-test-56
servers_res = session.get("http://127.0.0.1:8080/api/database/servers")
# Wait! Let's check what the GET /api/database/servers endpoint returns.
# If it is not that endpoint, we can just look up from the install response,
# or we can hit the list endpoint. Let's inspect list endpoint.
print("Servers status:", servers_res.status_code)
try:
    servers = servers_res.json().get("data", [])
except Exception:
    print("Failed to get servers list:", servers_res.text)
    servers = []

target_id = None
for s in servers:
    if s.get("name") == "mysql-test-56":
        target_id = s.get("id")
        break

if target_id is None:
    # Fallback to checking the install-server list by hitting it again
    # or using a query. Let's just assume we can find it.
    print("Could not find server mysql-test-56 in list, looking up via debug_servers...")
    # Let's hit install-server with empty body or another endpoint to get debug_servers.
    # Actually, we can just hit /api/database/servers/status with a dummy call or get the list.
    print("Available servers:", servers)
    # If list is empty, let's try ID 2
    target_id = 2

print(f"Targeting server ID: {target_id}")

# 3. Toggle Connection Pool
payload = {
    "id": target_id,
    "pool_enabled": 1
}
toggle_res = session.post(
    "http://127.0.0.1:8080/api/database/servers/toggle-pool",
    headers={"X-CSRF-Token": csrf_token},
    json=payload
)
print("Toggle status:", toggle_res.status_code)
print("Toggle response:", toggle_res.text)
