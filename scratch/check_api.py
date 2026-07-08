import jwt
import requests
import time

JWT_SECRET = "8c00ceea9e5e6316f8eb6dfe5b1d754f972aafca1c3d79e2dbc59a8621d1010b"

# Generate JWT token
payload = {
    "sub": "admin",
    "role": "admin",
    "exp": int(time.time()) + 3600
}
token = jwt.encode(payload, JWT_SECRET, algorithm="HS256")

headers = {
    "Authorization": f"Bearer {token}"
}

# 1. Get database servers
r = requests.get("http://localhost:8080/api/database/servers", headers=headers)
print("GET /api/database/servers status:", r.status_code)
try:
    print("GET /api/database/servers JSON:", r.json())
except Exception as e:
    print("GET /api/database/servers Text:", r.text)

# 2. Get me
r = requests.get("http://localhost:8080/api/auth/me", headers=headers)
print("GET /api/auth/me JSON:", r.json())
