import requests

paths = ["/login", "/zpanel", "/admin", "/control", "/panel", "/entrance"]
for p in paths:
    r = requests.get(f"http://localhost:8080{p}")
    print(f"GET {p} status: {r.status_code}")
    if r.status_code != 404:
        print(f"Found non-404 path: {p}")
