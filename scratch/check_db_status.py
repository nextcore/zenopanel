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

print("Waiting 10 seconds for database to finish initial startup...")
time.sleep(10)

# Check status
res = session.get(
    f"{base_url}/api/database/servers/status?name=mysql-test-57&driver=mysql",
    headers={"X-CSRF-Token": csrf_token}
)

print("Status code:", res.status_code)
try:
    print(json.dumps(res.json(), indent=2))
except Exception:
    print(res.text)
