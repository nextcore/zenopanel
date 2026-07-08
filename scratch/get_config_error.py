import requests
import re

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

# Fetch config
res_config = session.get(f"{base_url}/api/database/servers/config?id=1")
print(f"Status Code: {res_config.status_code}")
print("Response headers:")
for k, v in res_config.headers.items():
    print(f"  {k}: {v}")
print("\nResponse Body:")
print(res_config.text)
