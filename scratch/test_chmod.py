import os
import requests
import stat

BASE_URL = "http://localhost:3001"
TEST_FILE = "/home/max/Documents/PROJ/github/zenopanel/test-perm-temp.txt"

def main():
    print("--- Starting Permissions Integration Test ---")
    
    # 1. Create a temporary test file
    if os.path.exists(TEST_FILE):
        os.remove(TEST_FILE)
    with open(TEST_FILE, "w") as f:
        f.write("test permissions")
    
    initial_mode = os.stat(TEST_FILE).st_mode & 0o777
    print(f"1. Created temporary file. Initial permissions: {oct(initial_mode)}")

    # 2. Login to get session token (handling CSRF token first)
    session = requests.Session()
    
    # Get initial CSRF token
    get_resp = session.get(f"{BASE_URL}/login")
    csrf_token = session.cookies.get("_csrf")
    if not csrf_token:
        print("ERROR: Could not fetch CSRF token from /login cookies")
        return
        
    print(f"Fetched CSRF token: {csrf_token}")
    
    login_resp = session.post(f"{BASE_URL}/login", json={
        "username": "admin",
        "password": "admin"
    }, headers={
        "X-CSRF-Token": csrf_token
    })
    
    if login_resp.status_code != 200:
        print(f"ERROR: Login failed with status code {login_resp.status_code}")
        print(login_resp.text)
        return
    
    print("2. Logged in successfully.")

    # 3. Change permission to 0777 via API
    print("3. Changing permission to 777 via POST /api/files/chmod...")
    resp = session.post(f"{BASE_URL}/api/files/chmod", json={
        "path": TEST_FILE,
        "mode": "777",
        "recursive": False
    })
    
    if resp.status_code != 200:
        print(f"ERROR: API request failed with status code {resp.status_code}")
        print(resp.text)
        return
        
    print(f"API Response: {resp.json()}")

    # Verify actual permissions on disk
    new_mode = os.stat(TEST_FILE).st_mode & 0o777
    print(f"Actual permissions on disk: {oct(new_mode)}")
    assert new_mode == 0o777, f"Expected 0o777, got {oct(new_mode)}"
    print("Success: File permissions changed to 0o777 successfully!")

    # 4. Change permission back to 0644 via API
    print("4. Changing permission to 644 via POST /api/files/chmod...")
    resp = session.post(f"{BASE_URL}/api/files/chmod", json={
        "path": TEST_FILE,
        "mode": "644",
        "recursive": False
    })
    
    if resp.status_code != 200:
        print(f"ERROR: API request failed with status code {resp.status_code}")
        print(resp.text)
        return
        
    print(f"API Response: {resp.json()}")

    # Verify actual permissions on disk
    final_mode = os.stat(TEST_FILE).st_mode & 0o777
    print(f"Actual permissions on disk: {oct(final_mode)}")
    assert final_mode == 0o644, f"Expected 0o644, got {oct(final_mode)}"
    print("Success: File permissions changed back to 0o644 successfully!")

    # Clean up
    if os.path.exists(TEST_FILE):
        os.remove(TEST_FILE)
    print("5. Temporary test file cleaned up.")
    print("--- Permissions Integration Test Passed! ---")

if __name__ == "__main__":
    main()
