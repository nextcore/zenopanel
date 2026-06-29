import requests
import re

session = requests.Session()

# 1. Get login page to extract CSRF token (connecting to port 8080)
res = session.get("http://127.0.0.1:8080/login")
csrf_match = re.search(r"csrfToken = '([^']+)';", res.text)
if not csrf_match:
    print("Failed to find CSRF token in page")
    exit(1)
csrf_token = csrf_match.group(1)

# 2. Login
login_payload = {"username": "admin", "password": "admin"}
session.post(
    "http://127.0.0.1:8080/login",
    headers={"X-CSRF-Token": csrf_token},
    json=login_payload
)

# 3. Get logs for test-busybox
res = session.get("http://127.0.0.1:8080/api/box/logs?id=test-busybox")
print("=== test-busybox logs ===")
print(res.json())

# 4. Get logs for mysql-57
res = session.get("http://127.0.0.1:8080/api/box/logs?id=mysql-57")
print("=== mysql-57 logs ===")
print(res.json())
