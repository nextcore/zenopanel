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

# 2. Toggle Connection Pool
payload = {
    "id": 1,
    "pool_enabled": 1
}
toggle_res = session.post(
    "http://127.0.0.1:8080/api/database/servers/toggle-pool",
    headers={"X-CSRF-Token": csrf_token},
    json=payload
)
print("Toggle status:", toggle_res.status_code)
print("Toggle response:", toggle_res.text)
