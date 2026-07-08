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

# Inspect container
res = session.get(
    f"{base_url}/api/containers/inspect?id=mysql-test-57",
    headers={"X-CSRF-Token": csrf_token}
)

print("Status:", res.status_code)
try:
    print(json.dumps(res.json(), indent=2))
except Exception as e:
    print("Error parsing:", e)
    print(res.text)
