import sqlite3
import requests
import re

# 1. Temporarily change admin password to 'admin'
conn = sqlite3.connect("zeno.db")
cursor = conn.cursor()
cursor.execute("SELECT password_hash FROM users WHERE username='admin'")
orig_hash = cursor.fetchone()[0]
print("Original hash:", orig_hash)

new_hash = "$2b$12$lUTRhfJHIPGlrOK5E.kbo.QWFRtN7CDhxHpkDfU0qNRglFhxnTdJS"
cursor.execute("UPDATE users SET password_hash=? WHERE username='admin'", (new_hash,))
conn.commit()
conn.close()

try:
    session = requests.Session()
    
    # 2. Get /zpanel to get _csrf cookie and csrf token from response HTML
    r_get = session.get("http://localhost:8080/zpanel")
    print("GET /zpanel status:", r_get.status_code)
    
    csrf_cookie = session.cookies.get("_csrf")
    print("CSRF Cookie:", csrf_cookie)
    
    # Find CSRF token in HTML
    csrf_token_match = re.search(r'name="csrf_token"\s+value="([^"]+)"', r_get.text)
    csrf_token = csrf_token_match.group(1) if csrf_token_match else csrf_cookie
    print("CSRF Token parsed:", csrf_token)
    
    # 3. Post to login
    headers = {
        "X-CSRF-Token": csrf_token,
        "Referer": "http://localhost:8080/zpanel",
        "Content-Type": "application/json"
    }
    
    r_post = session.post("http://localhost:8080/zpanel", json={"username": "admin", "password": "admin"}, headers=headers)
    print("POST login status:", r_post.status_code)
    print("POST login response:", r_post.text)
    print("Session Cookies:", session.cookies.get_dict())
    
    # 4. Try calling /api/auth/me to verify we are logged in
    r_me = session.get("http://localhost:8080/api/auth/me")
    print("GET /api/auth/me status:", r_me.status_code)
    print("GET /api/auth/me JSON:", r_me.json())
    
finally:
    # Restore original hash
    conn = sqlite3.connect("zeno.db")
    cursor = conn.cursor()
    cursor.execute("UPDATE users SET password_hash=? WHERE username='admin'", (orig_hash,))
    conn.commit()
    conn.close()
    print("Restored original hash.")
