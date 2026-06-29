import requests
import json
import re

session = requests.Session()

# 1. Get CSRF Token
login_url = "http://127.0.0.1:3002/login"
print("Fetching login page for CSRF token...")
r_get = session.get(login_url)

csrf_token = ""
match = re.search(r"const\s+csrfToken\s*=\s*'([^']+)';", r_get.text)
if match:
    csrf_token = match.group(1)
    print("Found CSRF Token:", csrf_token)
else:
    print("Could not find CSRF token in HTML!")
    print(r_get.text[:1000])

# 2. Login
payload = {
    "username": "admin",
    "password": "admin"
}
headers = {
    "Content-Type": "application/json",
    "X-CSRF-Token": csrf_token
}

print("\nLogging in...")
r_login = session.post(login_url, json=payload, headers=headers)
print("Login status:", r_login.status_code)
print("Login response:", r_login.text)

# 3. Deploy & Register Database Engine
install_url = "http://127.0.0.1:3002/api/database/install-server"
db_payload = {
    "engine": "mysql:5.7",
    "name": "mysql-test-9",
    "port": 3316,
    "root_password": "Veteran31",
    "data_dir": "/var/lib/zenopanel/db/mysql-test-9"
}

headers_api = {
    "Content-Type": "application/json",
    "X-CSRF-Token": csrf_token
}

print("\nDeploying database server...")
try:
    r_install = session.post(install_url, json=db_payload, headers=headers_api, timeout=120)
    print("Install status:", r_install.status_code)
    print("Install response:")
    try:
        print(json.dumps(r_install.json(), indent=2))
    except Exception as e:
        print("Failed to parse JSON:", e)
        print("Raw text response:", r_install.text)
except Exception as e:
    print("Request failed:", e)
