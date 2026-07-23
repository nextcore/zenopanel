import subprocess
import time
import requests
import re
import os
import shutil

# 1. Create a isolated sandbox directory in scratch/test_run
sandbox_dir = "scratch/test_run"
if os.path.exists(sandbox_dir):
    shutil.rmtree(sandbox_dir)
os.makedirs(sandbox_dir)

# 2. Copy necessary assets
shutil.copy("dist/zenopanel-v1.7.12/zeno", sandbox_dir)
shutil.copytree("zsrc", os.path.join(sandbox_dir, "zsrc"))
shutil.copytree("views", os.path.join(sandbox_dir, "views"))
shutil.copytree("public", os.path.join(sandbox_dir, "public"))
shutil.copy("dist/zenopanel-v1.7.12/zeno.db", os.path.join(sandbox_dir, "zeno.db"))

# 3. Create .env with custom ports
with open("dist/zenopanel-v1.7.12/.env", "r") as f:
    env_content = f.read()

modified_env = env_content
modified_env = re.sub(r"APP_PORT=\S+", "APP_PORT=:8081", modified_env)
modified_env = re.sub(r"MGMT_PORT=\S+", "MGMT_PORT=:3003", modified_env)

with open(os.path.join(sandbox_dir, ".env"), "w") as f:
    f.write(modified_env)

print("Sandbox directory prepared.")

# Open a log file to avoid pipe buffer blocking
log_file = open(os.path.join(sandbox_dir, "zeno_run.log"), "w")

# 4. Start zeno process in scratch/test_run
process = subprocess.Popen(
    ["./zeno"],
    cwd=sandbox_dir,
    stdout=log_file,
    stderr=subprocess.STDOUT,
    text=True
)

# Wait 5 seconds for server to start
time.sleep(5)

print("Server started, logging in and querying status...")

try:
    session = requests.Session()
    # Login
    res = session.get("http://127.0.0.1:8081/login")
    csrf_token = re.search(r"csrfToken = '([^']+)';", res.text).group(1)
    
    session.post(
        "http://127.0.0.1:8081/login",
        headers={"X-CSRF-Token": csrf_token},
        json={"username": "admin", "password": "admin"}
    )
    
    # Query status
    res = session.get(
        "http://127.0.0.1:8081/api/database/servers/status?name=mysql-test-56&driver=mysql",
        headers={"X-CSRF-Token": csrf_token}
    )
    print("STATUS CODE:", res.status_code)
    print("RESPONSE BODY:", repr(res.text))

except Exception as e:
    print("Request failed:", e)

# 5. Wait a bit, terminate and capture all output
time.sleep(1)
process.terminate()
log_file.close()

# Read the log file
with open(os.path.join(sandbox_dir, "zeno_run.log"), "r") as f:
    logs = f.read()

print("\n--- SERVER LOGS ---")
print(logs)

# Clean up sandbox
if os.path.exists(sandbox_dir):
    shutil.rmtree(sandbox_dir)
