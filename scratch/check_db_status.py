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

# 2. Get Database Servers Status with query params
status_res = session.get(
    "http://127.0.0.1:8080/api/database/servers/status",
    params={"name": "mysql-test-56", "driver": "mysql"}
)
print("Status Response:")
try:
    print(json.dumps(status_res.json(), indent=2))
except Exception as e:
    print(status_res.text)
