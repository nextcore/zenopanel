package internal

import (
	"encoding/json"
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

// FindAvailableIP scans existing container states to pick a unique IP in the given subnet.
func FindAvailableIP(dataDir, subnet, gateway string) (string, error) {
	parts := strings.Split(subnet, ".")
	if len(parts) < 2 {
		return "", fmt.Errorf("invalid subnet format: %s", subnet)
	}
	x, err := strconv.Atoi(parts[1])
	if err != nil {
		return "", fmt.Errorf("invalid subnet number in %s: %w", subnet, err)
	}

	takenIPs := make(map[string]bool)
	takenIPs[gateway] = true // Bridge IP

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
		ip := fmt.Sprintf("172.%d.0.%d", x, i)
		if !takenIPs[ip] {
			return ip, nil
		}
	}

	return "", fmt.Errorf("no available IP addresses left in subnet %s", subnet)
}

// ConfigureContainerNetwork configures the namespaces and veth pairs for a container.
func ConfigureContainerNetwork(dataDir, containerID string, pid int, ports []string) (string, error) {
	bridgeID := "zenobr0"
	subnetStr := "172.20.0.0/16"
	gatewayIP := "172.20.0.1"

	cm := NewContainerManager(dataDir)
	state, err := cm.loadState(containerID)
	if err == nil && state.Network != "" && state.Network != "bridge" && state.Network != "default" {
		networks, err := LoadNetworks(dataDir)
		if err == nil {
			for _, n := range networks {
				if n.Name == state.Network || n.ID == state.Network {
					bridgeID = n.ID
					subnetStr = n.Subnet
					gatewayIP = n.Gateway
					break
				}
			}
		}
	}

	// Ensure bridge interface is created
	if bridgeID == "zenobr0" {
		if err := SetupBridge(); err != nil {
			return "", err
		}
	} else {
		// Verify custom bridge link exists
		_, err := net.InterfaceByName(bridgeID)
		if err != nil {
			return "", fmt.Errorf("custom bridge interface %s does not exist", bridgeID)
		}
	}

	ip, err := FindAvailableIP(dataDir, subnetStr, gatewayIP)
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

	// 2. Attach host end to bridge
	if err := exec.Command("ip", "link", "set", vethHost, "master", bridgeID).Run(); err != nil {
		return "", fmt.Errorf("failed to attach veth to bridge %s: %w", bridgeID, err)
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

	if err := exec.Command("nsenter", "-t", pidStr, "-n", "ip", "route", "add", "default", "via", gatewayIP).Run(); err != nil {
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

// SyncHostsEntries updates /etc/hosts in all running containers to enable name-based communication.
func SyncHostsEntries(dataDir string) error {
	cm := NewContainerManager(dataDir)
	containers, err := cm.ContainerList()
	if err != nil {
		return err
	}

	// 1. Collect network and IP for all running containers
	runningIPs := make(map[string]string)
	runningNetworks := make(map[string]string)
	for _, c := range containers {
		if c.Status == StatusRunning && c.Env != nil {
			if ip, ok := c.Env["ZENO_IP"]; ok && ip != "" {
				runningIPs[c.ID] = ip
				runningNetworks[c.ID] = c.Network
			}
		}
	}

	// 2. For each running container, write loopback + mappings of other containers on the SAME network
	for _, c := range containers {
		if c.Status != StatusRunning {
			continue
		}

		hostsPath := RootfsDir(dataDir, c.ID) + "/etc/hosts"

		var sb strings.Builder
		sb.WriteString("127.0.0.1\tlocalhost\n")
		sb.WriteString("::1\tlocalhost ip6-localhost ip6-loopback\n\n")
		sb.WriteString("# Zeno Container Service Discovery\n")

		// Add mapping for this container's own IP
		if myIP, ok := runningIPs[c.ID]; ok {
			sb.WriteString(fmt.Sprintf("%s\t%s\n", myIP, c.ID))
		}

		// Add mapping for all OTHER running containers ON THE SAME NETWORK
		for otherID, otherIP := range runningIPs {
			if otherID != c.ID && runningNetworks[otherID] == c.Network {
				sb.WriteString(fmt.Sprintf("%s\t%s\n", otherIP, otherID))
			}
		}

		_ = os.WriteFile(hostsPath, []byte(sb.String()), 0644)
	}

	return nil
}

// LoadNetworks loads networks from networks.json.
func LoadNetworks(dataDir string) ([]NetworkConfig, error) {
	path := dataDir + "/networks.json"
	if _, err := os.Stat(path); os.IsNotExist(err) {
		return []NetworkConfig{}, nil
	}
	data, err := os.ReadFile(path)
	if err != nil {
		return nil, err
	}
	var networks []NetworkConfig
	if err := json.Unmarshal(data, &networks); err != nil {
		return nil, err
	}
	return networks, nil
}

// SaveNetworks saves networks to networks.json.
func SaveNetworks(dataDir string, networks []NetworkConfig) error {
	path := dataDir + "/networks.json"
	data, err := json.MarshalIndent(networks, "", "  ")
	if err != nil {
		return err
	}
	return os.WriteFile(path, data, 0644)
}

// CreateBridgeNetwork creates a new bridge network on the host and stores it in networks.json.
func CreateBridgeNetwork(dataDir, name string) error {
	networks, err := LoadNetworks(dataDir)
	if err != nil {
		return err
	}

	for _, n := range networks {
		if n.Name == name {
			return fmt.Errorf("network %s already exists", name)
		}
	}

	if name == "bridge" || name == "default" {
		return fmt.Errorf("network name '%s' is reserved", name)
	}

	usedSubnets := make(map[int]bool)
	for _, n := range networks {
		parts := strings.Split(n.Subnet, ".")
		if len(parts) > 1 {
			if x, err := strconv.Atoi(parts[1]); err == nil {
				usedSubnets[x] = true
			}
		}
	}

	selectedX := -1
	for x := 21; x <= 31; x++ {
		if !usedSubnets[x] {
			selectedX = x
			break
		}
	}

	if selectedX == -1 {
		return fmt.Errorf("no available subnets in range 172.21.0.0/16 - 172.31.0.0/16")
	}

	bridgeID := fmt.Sprintf("zenobr%d", selectedX)
	subnet := fmt.Sprintf("172.%d.0.0/16", selectedX)
	gateway := fmt.Sprintf("172.%d.0.1", selectedX)

	if err := exec.Command("ip", "link", "add", "name", bridgeID, "type", "bridge").Run(); err != nil {
		return fmt.Errorf("failed to create bridge %s: %w", bridgeID, err)
	}

	if err := exec.Command("ip", "addr", "add", gateway+"/16", "dev", bridgeID).Run(); err != nil {
		_ = exec.Command("ip", "link", "delete", bridgeID).Run()
		return fmt.Errorf("failed to assign IP to bridge %s: %w", bridgeID, err)
	}

	if err := exec.Command("ip", "link", "set", bridgeID, "up").Run(); err != nil {
		_ = exec.Command("ip", "link", "delete", bridgeID).Run()
		return fmt.Errorf("failed to bring bridge %s up: %w", bridgeID, err)
	}

	_ = exec.Command("iptables", "-t", "nat", "-A", "POSTROUTING", "-s", subnet, "!", "-o", bridgeID, "-j", "MASQUERADE").Run()

	newNet := NetworkConfig{
		ID:      bridgeID,
		Name:    name,
		Driver:  "bridge",
		Subnet:  subnet,
		Gateway: gateway,
	}
	networks = append(networks, newNet)
	if err := SaveNetworks(dataDir, networks); err != nil {
		_ = exec.Command("ip", "link", "delete", bridgeID).Run()
		_ = exec.Command("iptables", "-t", "nat", "-D", "POSTROUTING", "-s", subnet, "!", "-o", bridgeID, "-j", "MASQUERADE").Run()
		return err
	}

	return nil
}

// DeleteBridgeNetwork removes a bridge network from the host and networks.json.
func DeleteBridgeNetwork(dataDir, name string) error {
	networks, err := LoadNetworks(dataDir)
	if err != nil {
		return err
	}

	foundIdx := -1
	var netToDelete NetworkConfig
	for i, n := range networks {
		if n.Name == name || n.ID == name {
			foundIdx = i
			netToDelete = n
			break
		}
	}

	if foundIdx == -1 {
		return fmt.Errorf("network %s not found", name)
	}

	cm := NewContainerManager(dataDir)
	containers, err := cm.ContainerList()
	if err == nil {
		for _, c := range containers {
			if c.Network == netToDelete.Name && (c.Status == StatusRunning || c.Status == StatusCreated) {
				return fmt.Errorf("network is in use by container %s", c.ID)
			}
		}
	}

	_ = exec.Command("ip", "link", "set", netToDelete.ID, "down").Run()

	if err := exec.Command("ip", "link", "delete", netToDelete.ID).Run(); err != nil {
		return fmt.Errorf("failed to delete bridge %s: %w", netToDelete.ID, err)
	}

	_ = exec.Command("iptables", "-t", "nat", "-D", "POSTROUTING", "-s", netToDelete.Subnet, "!", "-o", netToDelete.ID, "-j", "MASQUERADE").Run()

	networks = append(networks[:foundIdx], networks[foundIdx+1:]...)
	return SaveNetworks(dataDir, networks)
}
