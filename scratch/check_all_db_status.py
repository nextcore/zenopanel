import requests
import re

session = requests.Session()

# 1. Login to ZenoPanel on port 8080
res = session.get("http://127.0.0.1:8080/login")
csrf_token = re.search(r"csrfToken = '([^']+)';", res.text).group(1)

login_res = session.post(
    "http://127.0.0.1:8080/login",
    headers={"X-CSRF-Token": csrf_token},
    json={"username": "admin", "password": "admin"}
)

# 2. Get registered servers
servers_res = session.get("http://127.0.0.1:8080/api/database/servers")
servers = servers_res.json().get("data", [])

print(f"Found {len(servers)} registered servers.")
for s in servers:
    srv_id = s.get("id")
    srv_name = s.get("name")
    srv_driver = s.get("driver")
    
    # Check status with correct query parameters: name and driver
    status_res = session.get(f"http://127.0.0.1:8080/api/database/servers/status?name={srv_name}&driver={srv_driver}")
    print(f"Server: {srv_name} (ID: {srv_id}, Driver: {srv_driver})")
    print("Status:", status_res.text)
