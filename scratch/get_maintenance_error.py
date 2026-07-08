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
res_login = session.post(
    f"{base_url}/login",
    headers={"X-CSRF-Token": csrf_token},
    json={"username": "admin", "password": "admin"}
)
print(f"Login Status Code: {res_login.status_code}")
print(f"Login Response Body: {res_login.text[:200]}")

# Fetch maintenance check
res_maint = session.post(
    f"{base_url}/api/database/maintenance",
    headers={"X-CSRF-Token": csrf_token},
    json={
        "server_name": "mysql-test-57",
        "db_name": "test_db",
        "driver": "mysql",
        "action": "check"
    }
)
print(f"Status Code: {res_maint.status_code}")
print("Response headers:")
for k, v in res_maint.headers.items():
    print(f"  {k}: {v}")
print("\nResponse Body:")
print(res_maint.text)
