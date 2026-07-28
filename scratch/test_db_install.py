import base64
import hashlib
import hmac
import json
import time
import urllib.request
import urllib.error

# Read JWT_SECRET from dist env
jwt_secret = None
with open('dist/zenopanel-v1.8.0/.env', 'r') as f:
    for line in f:
        if line.startswith('JWT_SECRET='):
            jwt_secret = line.strip().split('=', 1)[1]
            break

if not jwt_secret:
    print("Error: JWT_SECRET not found in .env")
    exit(1)

# Generate JWT Token using HS256 (Pure Python)
def base64url_encode(data: bytes) -> str:
    return base64.urlsafe_b64encode(data).rstrip(b'=').decode('utf-8')

header = base64url_encode(json.dumps({"alg": "HS256", "typ": "JWT"}).encode('utf-8'))
payload = base64url_encode(json.dumps({
    "sub": "admin",
    "role": "admin",
    "exp": int(time.time()) + 3600
}).encode('utf-8'))

signature = hmac.new(
    jwt_secret.encode('utf-8'),
    f"{header}.{payload}".encode('utf-8'),
    hashlib.sha256
).digest()

token = f"{header}.{payload}.{base64url_encode(signature)}"
print(f"Generated JWT Token: {token}")

# Call API to install mysql56
url = "http://localhost:8080/api/database/install-server"
data = {
    "engine": "mysql:5.6",
    "name": "mysql56",
    "port": 3306,
    "root_password": "SM6mwSqbFFGRbviZ",
    "data_dir": "mysql56_data",
    "is_remote": 0,
    "mem_limit": "",
    "cpus": "",
    "pool_enabled": 0,
    "pool_port": 0
}

req = urllib.request.Request(
    url,
    data=json.dumps(data).encode('utf-8'),
    headers={
        "Content-Type": "application/json",
        "Authorization": f"Bearer {token}"
    },
    method="POST"
)

try:
    print("Sending POST request to install-server...")
    with urllib.request.urlopen(req) as response:
        print("Status Code:", response.getcode())
        print("Response Body:", response.read().decode('utf-8'))
except urllib.error.HTTPError as e:
    print("HTTP Error:", e.code)
    print("Response:", e.read().decode('utf-8'))
except Exception as e:
    print("Error:", e)
