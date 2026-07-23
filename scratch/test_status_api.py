import requests
import re
import json

session = requests.Session()
base_url = "http://127.0.0.1:8080"

# 1. Get Login Page to obtain CSRF
res = session.get(f"{base_url}/login")
res.raise_for_status()
csrf_token = re.search(r"csrfToken = '([^']+)';", res.text).group(1)

# 2. Login
session.post(
    f"{base_url}/login",
    headers={"X-CSRF-Token": csrf_token},
    json={"username": "admin", "password": "admin"}
)

# 3. Request status
res = session.get(
    f"{base_url}/api/database/servers/status?name=mysql-test-56&driver=mysql",
    headers={"X-CSRF-Token": csrf_token}
)
print("STATUS CODE:", res.status_code)
print("HEADERS:", res.headers)
print("BODY:", repr(res.text))
