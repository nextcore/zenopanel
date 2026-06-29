import requests
import re
import json

session = requests.Session()

# 1. Get login page to extract CSRF token (connecting to port 8080)
res = session.get("http://127.0.0.1:8080/login")
csrf_match = re.search(r"csrfToken = '([^']+)';", res.text)
if not csrf_match:
    print("Failed to find CSRF token in page")
    exit(1)
csrf_token = csrf_match.group(1)
print(f"Extracted CSRF token: {csrf_token}")

# 2. Login
login_payload = {"username": "admin", "password": "admin"}
res = session.post(
    "http://127.0.0.1:8080/login",
    headers={"X-CSRF-Token": csrf_token},
    json=login_payload
)
print("Login status:", res.status_code)

# 3. Pull busybox
pull_payload = {"image": "busybox:latest"}
print("Pulling busybox:latest...")
res = session.post(
    "http://127.0.0.1:8080/api/box/pull",
    headers={"X-CSRF-Token": csrf_token},
    json=pull_payload
)
print("Pull status:", res.status_code)
print("Pull response:", res.text)

# 4. Create test container
create_payload = {
    "name": "test-busybox",
    "image": "busybox:latest",
    "cmd": ["sleep", "3600"],
    "ports": [],
    "volumes": [],
    "env": {},
    "host_net": False,
    "memory": 0,
    "cpus": 0.0,
    "oom_score_adj": 0,
    "read_only": False
}
print("Creating container test-busybox...")
res = session.post(
    "http://127.0.0.1:8080/api/box/create",
    headers={"X-CSRF-Token": csrf_token},
    json=create_payload
)
print("Create status:", res.status_code)
print("Create response:", res.text)

# 5. Start container
start_payload = {"id": "test-busybox"}
print("Starting container test-busybox...")
res = session.post(
    "http://127.0.0.1:8080/api/box/start",
    headers={"X-CSRF-Token": csrf_token},
    json=start_payload
)
print("Start status:", res.status_code)
print("Start response:", res.text)
