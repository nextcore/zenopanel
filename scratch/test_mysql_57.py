import requests
import re
import json

session = requests.Session()

# Try ports 8080 first
base_url = "http://127.0.0.1:8080"
print(f"Trying base URL: {base_url}")

try:
    res = session.get(f"{base_url}/login")
    res.raise_for_status()
except Exception as e:
    base_url = "http://127.0.0.1:3002"
    print(f"Port 8080 failed, trying base URL: {base_url}")
    res = session.get(f"{base_url}/login")

# Extract CSRF token
csrf_match = re.search(r"csrfToken = '([^']+)';", res.text)
if not csrf_match:
    # Try another pattern just in case
    csrf_match = re.search(r"const\s+csrfToken\s*=\s*'([^']+)';", res.text)

if not csrf_match:
    print("Failed to find CSRF token in page")
    print(res.text[:1000])
    exit(1)

csrf_token = csrf_match.group(1)
print(f"Extracted CSRF token: {csrf_token}")

# Login
login_payload = {"username": "admin", "password": "admin"}
res = session.post(
    f"{base_url}/login",
    headers={"X-CSRF-Token": csrf_token},
    json=login_payload
)
print("Login status:", res.status_code)

# Install mysql:5.7 on port 3309
install_payload = {
    "engine": "mysql:5.7",
    "name": "mysql-test-57",
    "port": 3309,
    "root_password": "zenopanel-mysql57-pass",
    "data_dir": "/var/lib/zenopanel/db/mysql-test-57"
}
res = session.post(
    f"{base_url}/api/database/install-server",
    headers={"X-CSRF-Token": csrf_token},
    json=install_payload
)
print("Install status:", res.status_code)
print("Install response:")
try:
    print(json.dumps(res.json(), indent=2))
except Exception:
    print(res.text)
