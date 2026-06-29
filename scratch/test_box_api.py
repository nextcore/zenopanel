import requests
import re
import json

session = requests.Session()

# 1. Get login page to extract CSRF token (connecting to port 8080)
res = session.get("http://127.0.0.1:8080/login")
csrf_match = re.search(r"csrfToken = '([^']+)';", res.text)
if not csrf_match:
    print("Failed to find CSRF token in page")
    exit(1)
csrf_token = csrf_match.group(1)
print(f"Extracted CSRF token: {csrf_token}")

# 2. Login
login_payload = {"username": "admin", "password": "admin"}
res = session.post(
    "http://127.0.0.1:8080/login",
    headers={"X-CSRF-Token": csrf_token},
    json=login_payload
)
print("Login status:", res.status_code)

# 3. Get images
res = session.get("http://127.0.0.1:8080/api/box/images")
print("Images status:", res.status_code)
print("Images list:", res.json())

# 4. Get containers list
res = session.get("http://127.0.0.1:8080/api/box/list")
print("Containers status:", res.status_code)
print("Containers list:", res.json())
