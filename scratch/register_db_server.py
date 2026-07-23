import requests
import re

session = requests.Session()

# 1. Login to ZenoPanel on port 8080
res = session.get("http://127.0.0.1:8080/login")
csrf_token = re.search(r"csrfToken = '([^']+)';", res.text).group(1)

login_res = session.post(
    "http://127.0.0.1:8080/login",
    headers={"X-CSRF-Token": csrf_token},
    json={"username": "admin", "password": "admin"}
)
print("Login status:", login_res.status_code)

# 2. Register / Install MySQL 5.6 database server (pool_enabled = 0)
payload = {
    "engine": "mysql:5.6",
    "name": "mysql-test-56",
    "port": 3312,
    "root_password": "zenopanel-mysql56-pass",
    "data_dir": "/var/lib/mysql-test-56",
    "is_remote": 0,
    "pool_enabled": 0,
    "pool_port": 6036
}

install_res = session.post(
    "http://127.0.0.1:8080/api/database/install-server",
    headers={"X-CSRF-Token": csrf_token},
    json=payload
)
print("Install status:", install_res.status_code)
print("Install response:", install_res.text)
