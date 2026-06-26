import requests
import re
import json

session = requests.Session()

# 1. Get login page to extract CSRF token (connecting directly to port 80)
res = session.get("http://127.0.0.1/login")
csrf_match = re.search(r"csrfToken = '([^']+)';", res.text)
if not csrf_match:
    print("Failed to find CSRF token in page")
    exit(1)
csrf_token = csrf_match.group(1)
print(f"Extracted CSRF token: {csrf_token}")

# 2. Login
login_payload = {"username": "admin", "password": "admin"}
res = session.post(
    "http://127.0.0.1/login",
    headers={"X-CSRF-Token": csrf_token},
    json=login_payload
)
print("Login status:", res.status_code)

# 3. Post to install-server
install_payload = {
    "engine": "mysql:8.4",
    "name": "mysql-84",
    "port": 3308,
    "root_password": "zenopanel",
    "data_dir": "/var/lib/zenopanel/db/mysql-84"
}
res = session.post(
    "http://127.0.0.1/api/database/install-server",
    headers={"X-CSRF-Token": csrf_token},
    json=install_payload
)
print("Install status:", res.status_code)
print("Install response:")
try:
    print(json.dumps(res.json(), indent=2))
except Exception:
    print(res.text)
