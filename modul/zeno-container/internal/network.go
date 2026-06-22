package internal

import (
	"fmt"
	"net"
	"os"
	"os/exec"
	"strconv"
	"strings"
)

// SetupBridge ensures the host bridge zenobr0 is created and configured.
func SetupBridge() error {
	// Check if zenobr0 exists
	_, err := net.InterfaceByName("zenobr0")
	if err == nil {
		return nil // Already exists
	}

	// 1. Create bridge
	if err := exec.Command("ip", "link", "add", "name", "zenobr0", "type", "bridge").Run(); err != nil {
		return fmt.Errorf("failed to create bridge zenobr0: %w", err)
	}

	// 2. Assign IP address
	if err := exec.Command("ip", "addr", "add", "172.20.0.1/16", "dev", "zenobr0").Run(); err != nil {
		return fmt.Errorf("failed to assign IP to zenobr0: %w", err)
	}

	// 3. Bring bridge up
	if err := exec.Command("ip", "link", "set", "zenobr0", "up").Run(); err != nil {
		return fmt.Errorf("failed to bring zenobr0 up: %w", err)
	}

	// 4. Configure NAT (iptables) masquerade
	checkCmd := exec.Command("iptables", "-t", "nat", "-C", "POSTROUTING", "-s", "172.20.0.0/16", "!", "-o", "zenobr0", "-j", "MASQUERADE")
	if err := checkCmd.Run(); err != nil {
		// Rule does not exist, append it
		_ = exec.Command("iptables", "-t", "nat", "-A", "POSTROUTING", "-s", "172.20.0.0/16", "!", "-o", "zenobr0", "-j", "MASQUERADE").Run()
	}

	return nil
}

// FindAvailableIP scans existing container states to pick a unique IP in 172.20.0.0/16.
func FindAvailableIP(dataDir string) (string, error) {
	takenIPs := make(map[string]bool)
	takenIPs["172.20.0.1"] = true // Bridge IP

	cm := NewContainerManager(dataDir)
	containers, err := cm.ContainerList()
	if err == nil {
		for _, c := range containers {
			if c.Env != nil {
				if ip, ok := c.Env["ZENO_IP"]; ok {
					takenIPs[ip] = true
				}
			}
		}
	}

	for i := 2; i < 255; i++ {
		ip := fmt.Sprintf("172.20.0.%d", i)
		if !takenIPs[ip] {
			return ip, nil
		}
	}

	return "", fmt.Errorf("no available IP addresses left in 172.20.0.0/16 subnet")
}

// ConfigureContainerNetwork configures the namespaces and veth pairs for a container.
func ConfigureContainerNetwork(dataDir, containerID string, pid int, ports []string) (string, error) {
	if err := SetupBridge(); err != nil {
		return "", err
	}

	ip, err := FindAvailableIP(dataDir)
	if err != nil {
		return "", err
	}

	vethHost := fmt.Sprintf("veth-h-%s", containerID)
	vethGuest := fmt.Sprintf("veth-g-%s", containerID)
	// Truncate interface names to max 15 chars (Linux constraint)
	if len(vethHost) > 15 {
		vethHost = vethHost[:15]
	}
	if len(vethGuest) > 15 {
		vethGuest = vethGuest[:15]
	}

	// Clean up any old veth pair if left over
	_ = exec.Command("ip", "link", "delete", vethHost).Run()

	// 1. Create veth pair
	if err := exec.Command("ip", "link", "add", vethHost, "type", "veth", "peer", "name", vethGuest).Run(); err != nil {
		return "", fmt.Errorf("failed to create veth pair: %w", err)
	}

	// 2. Attach host end to bridge zenobr0
	if err := exec.Command("ip", "link", "set", vethHost, "master", "zenobr0").Run(); err != nil {
		return "", fmt.Errorf("failed to attach veth to bridge: %w", err)
	}

	// 3. Bring host end up
	if err := exec.Command("ip", "link", "set", vethHost, "up").Run(); err != nil {
		return "", fmt.Errorf("failed to bring host veth up: %w", err)
	}

	// 4. Move guest end to container network namespace
	pidStr := strconv.Itoa(pid)
	if err := exec.Command("ip", "link", "set", vethGuest, "netns", pidStr).Run(); err != nil {
		return "", fmt.Errorf("failed to move veth to container netns: %w", err)
	}

	// 5. Configure interface inside the container namespace
	if err := exec.Command("nsenter", "-t", pidStr, "-n", "ip", "link", "set", vethGuest, "name", "eth0").Run(); err != nil {
		return "", fmt.Errorf("failed to rename interface to eth0 inside container: %w", err)
	}

	if err := exec.Command("nsenter", "-t", pidStr, "-n", "ip", "addr", "add", ip+"/16", "dev", "eth0").Run(); err != nil {
		return "", fmt.Errorf("failed to assign IP inside container: %w", err)
	}

	if err := exec.Command("nsenter", "-t", pidStr, "-n", "ip", "link", "set", "eth0", "up").Run(); err != nil {
		return "", fmt.Errorf("failed to bring eth0 up inside container: %w", err)
	}

	if err := exec.Command("nsenter", "-t", pidStr, "-n", "ip", "route", "add", "default", "via", "172.20.0.1").Run(); err != nil {
		return "", fmt.Errorf("failed to set gateway inside container: %w", err)
	}

	// Configure DNS in container /etc/resolv.conf
	resolvPath := RootfsDir(dataDir, containerID) + "/etc/resolv.conf"
	_ = os.WriteFile(resolvPath, []byte("nameserver 8.8.8.8\nnameserver 1.1.1.1\n"), 0644)

	// Set up Port Forwarding / DNAT for exposed ports on the host
	for _, p := range ports {
		parts := strings.SplitN(p, ":", 2)
		if len(parts) == 2 {
			hostPort := parts[0]
			containerPort := parts[1]

			_ = exec.Command("iptables", "-t", "nat", "-A", "PREROUTING", "-p", "tcp", "--dport", hostPort, "-j", "DNAT", "--to-destination", ip+":"+containerPort).Run()
			_ = exec.Command("iptables", "-t", "nat", "-A", "OUTPUT", "-p", "tcp", "--dport", hostPort, "-j", "DNAT", "--to-destination", ip+":"+containerPort).Run()
		}
	}

	return ip, nil
}

// CleanContainerNetwork removes NAT port forwarding rules when container is stopped/deleted.
func CleanContainerNetwork(containerID string, ip string, ports []string) {
	for _, p := range ports {
		parts := strings.SplitN(p, ":", 2)
		if len(parts) == 2 {
			hostPort := parts[0]
			containerPort := parts[1]

			_ = exec.Command("iptables", "-t", "nat", "-D", "PREROUTING", "-p", "tcp", "--dport", hostPort, "-j", "DNAT", "--to-destination", ip+":"+containerPort).Run()
			_ = exec.Command("iptables", "-t", "nat", "-D", "OUTPUT", "-p", "tcp", "--dport", hostPort, "-j", "DNAT", "--to-destination", ip+":"+containerPort).Run()
		}
	}
}
