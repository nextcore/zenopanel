import socket

def test_conn(ip, port):
    s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    s.settimeout(3)
    try:
        s.connect((ip, port))
        print(f"Successfully connected to {ip}:{port}!")
        try:
            banner = s.recv(1024)
            print("Received banner:", banner)
        except Exception as e:
            print("No banner / recv failed:", e)
    except Exception as e:
        print(f"Failed to connect to {ip}:{port}: {e}")
    finally:
        s.close()

print("--- Testing Direct Container IP ---")
test_conn("172.20.0.2", 3306)

print("\n--- Testing Port Forwarding on localhost ---")
test_conn("127.0.0.1", 3309)
