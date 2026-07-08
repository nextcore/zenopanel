import sqlite3
import requests

# Backup the original password hash
conn = sqlite3.connect("zeno.db")
cursor = conn.cursor()
cursor.execute("SELECT password_hash FROM users WHERE username='admin'")
orig_hash = cursor.fetchone()[0]
print("Original hash:", orig_hash)

# Update to bcrypt of 'admin'
new_hash = "$2b$12$lUTRhfJHIPGlrOK5E.kbo.QWFRtN7CDhxHpkDfU0qNRglFhxnTdJS"
cursor.execute("UPDATE users SET password_hash=? WHERE username='admin'", (new_hash,))
conn.commit()
conn.close()

try:
    # Try logging in
    login_url = "http://localhost:8080/zpanel"
    r = requests.post(login_url, json={"username": "admin", "password": "admin"})
    print("Login status:", r.status_code)
    print("Login response:", r.text)
    print("Cookies:", r.cookies.get_dict())
finally:
    # Restore original hash
    conn = sqlite3.connect("zeno.db")
    cursor = conn.cursor()
    cursor.execute("UPDATE users SET password_hash=? WHERE username='admin'", (orig_hash,))
    conn.commit()
    conn.close()
    print("Restored original hash.")
