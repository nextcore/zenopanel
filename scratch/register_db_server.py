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

# 2. Register / Install MySQL 5.7 database server with Named Volume (no slash)
payload = {
    "engine": "mysql:5.7",
    "name": "mysql-test-57-vol2",
    "port": 3316,
    "root_password": "zenopanel-mysql57-pass",
    "data_dir": "mysql-test-57-vol2",
    "is_remote": 0,
    "pool_enabled": 0,
    "pool_port": 6040
}

install_res = session.post(
    "http://127.0.0.1:8080/api/database/install-server",
    headers={"X-CSRF-Token": csrf_token},
    json=payload
)
print("Install status:", install_res.status_code)
print("Install response:", install_res.text)
