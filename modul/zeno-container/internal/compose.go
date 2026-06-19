package internal

import (
	"fmt"
	"os"
	"sort"
	"strings"

	"gopkg.in/yaml.v3"
)

// ComposeService represents a service in docker-compose.yml
type ComposeService struct {
	Image         string            `yaml:"image"`
	ContainerName string            `yaml:"container_name"`
	Ports         []string          `yaml:"ports"`
	Environment   map[string]string `yaml:"environment"`
	Volumes       []string          `yaml:"volumes"`
	Command       string            `yaml:"command"`
	DependsOn     []string          `yaml:"depends_on"`
	Networks      []string          `yaml:"networks"`
}

// ComposeNetwork represents a named network in docker-compose.yml
type ComposeNetwork struct {
	Driver string `yaml:"driver"`
}

// ComposeFile represents the full docker-compose.yml
type ComposeFile struct {
	Version  string                    `yaml:"version"`
	Services map[string]ComposeService `yaml:"services"`
	Networks map[string]ComposeNetwork `yaml:"networks"`
}

// ComposeUpResult holds the result of bringing up a single service.
type ComposeUpResult struct {
	Service string
	Error   error
}

// ParseComposeFile reads and parses a docker-compose.yml file
func ParseComposeFile(path string) (*ComposeFile, error) {
	data, err := os.ReadFile(path)
	if err != nil {
		return nil, fmt.Errorf("read compose file: %w", err)
	}
	var cf ComposeFile
	if err := yaml.Unmarshal(data, &cf); err != nil {
		return nil, fmt.Errorf("parse compose file: %w", err)
	}
	return &cf, nil
}

// injectHostsEntries adds service discovery entries to container's /etc/hosts
func injectHostsEntries(dataDir, containerID string, services map[string]ComposeService, currentName string) error {
	hostsPath := RootfsDir(dataDir, containerID) + "/etc/hosts"

	// Read existing hosts
	data, err := os.ReadFile(hostsPath)
	if err != nil {
		// File might not exist, create it
		data = []byte("127.0.0.1 localhost\n")
	}

	// Build hosts entries for all services (including self)
	var entries []string
	for svcName, svc := range services {
		if svcName == currentName {
			continue
		}
		cn := svc.ContainerName
		if cn == "" {
			cn = svcName
		}
		// Use host networking — all containers accessible via localhost on different ports
		// Map service name to 127.0.0.1 for inter-container communication
		entries = append(entries, fmt.Sprintf("127.0.0.1\t%s\t%s", cn, svcName))
	}

	if len(entries) == 0 {
		return nil
	}

	// Add entries after existing content
	newData := append(data, []byte("\n# ZenoPanel compose service discovery\n")...)
	for _, e := range entries {
		newData = append(newData, []byte(e+"\n")...)
	}

	return os.WriteFile(hostsPath, newData, 0644)
}

// ComposeUp parses a compose file and creates/starts all services.
// Returns per-service results.
func (cm *ContainerManager) ComposeUp(path string) ([]ComposeUpResult, error) {
	cf, err := ParseComposeFile(path)
	if err != nil {
		return nil, err
	}

	// Order services respecting depends_on
	ordered := orderServices(cf.Services)

	var results []ComposeUpResult
	for _, name := range ordered {
		svc := cf.Services[name]
		fmt.Printf("▶ Service: %s (image: %s)\n", name, svc.Image)

		// 1. Ensure image is available (pull if needed)
		if svc.Image != "" {
			_, err := cm.ResolveImageCmd(svc.Image)
			if err != nil {
				results = append(results, ComposeUpResult{Service: name, Error: fmt.Errorf("resolve image: %w", err)})
				continue
			}
		}

		// 2. Determine container name
		containerName := svc.ContainerName
		if containerName == "" {
			containerName = name
		}

		// 3. Build arguments for ContainerCreate
		var cmdArgs []string
		if svc.Command != "" {
			cmdArgs = strings.Fields(svc.Command)
		}

		env := svc.Environment
		if env == nil {
			env = make(map[string]string)
		}

		volumes := svc.Volumes
		if volumes == nil {
			volumes = []string{}
		}

		ports := svc.Ports
		if ports == nil {
			ports = []string{}
		}

		// 4. Create container (using host networking by default for compose)
		fmt.Printf("  ▶ Creating container '%s'...\n", containerName)
		if err := cm.ContainerCreate(containerName, svc.Image, cmdArgs, env, "", volumes, ports, true); err != nil {
			results = append(results, ComposeUpResult{Service: name, Error: fmt.Errorf("create: %w", err)})
			continue
		}

		// 4b. Inject hosts entries for service discovery
		if err := injectHostsEntries(cm.DataDir, containerName, cf.Services, name); err != nil {
			fmt.Printf("  ⚠ Warning: could not inject hosts: %v\n", err)
		}

		// 5. Start container
		fmt.Printf("  ▶ Starting container '%s'...\n", containerName)
		if err := cm.ContainerStart(containerName); err != nil {
			results = append(results, ComposeUpResult{Service: name, Error: fmt.Errorf("start: %w", err)})
			continue
		}

		fmt.Printf("  ✓ Service '%s' is up.\n", name)
		results = append(results, ComposeUpResult{Service: name})
	}

	return results, nil
}

// ComposeDown stops and removes all services defined in a compose file.
func (cm *ContainerManager) ComposeDown(path string) error {
	cf, err := ParseComposeFile(path)
	if err != nil {
		return err
	}

	// Reverse dependency order for shutdown (dependants first)
	ordered := orderServices(cf.Services)
	for i := len(ordered) - 1; i >= 0; i-- {
		name := ordered[i]
		svc := cf.Services[name]

		containerName := svc.ContainerName
		if containerName == "" {
			containerName = name
		}

		fmt.Printf("  ▶ Stopping container '%s'...\n", containerName)
		if err := cm.ContainerStop(containerName); err != nil {
			fmt.Fprintf(os.Stderr, "  ⚠ Error stopping %s: %v\n", containerName, err)
		}

		fmt.Printf("  ▶ Removing container '%s'...\n", containerName)
		if err := cm.ContainerDelete(containerName); err != nil {
			fmt.Fprintf(os.Stderr, "  ⚠ Error removing %s: %v\n", containerName, err)
		}
	}

	return nil
}

// ComposePs lists containers that are managed by the given compose file.
func (cm *ContainerManager) ComposePs(path string) error {
	cf, err := ParseComposeFile(path)
	if err != nil {
		return err
	}

	containers, err := cm.ContainerList()
	if err != nil {
		return err
	}

	// Build a set of expected container names -> service name
	expected := make(map[string]string)
	for svcName, svc := range cf.Services {
		cn := svc.ContainerName
		if cn == "" {
			cn = svcName
		}
		expected[cn] = svcName
	}

	// Filter containers matching services
	var matched []ContainerState
	for _, c := range containers {
		if _, ok := expected[c.ID]; ok {
			matched = append(matched, c)
		}
	}

	if len(matched) == 0 {
		fmt.Println("No containers found for this compose file.")
		return nil
	}

	fmt.Printf("%-8s %-24s %-24s %-10s %-8s %s\n", "SERVICE", "CONTAINER", "IMAGE", "STATUS", "PID", "PORTS")
	fmt.Println(strings.Repeat("-", 110))
	for _, c := range matched {
		svcName := expected[c.ID]
		ports := strings.Join(c.Ports, ",")
		if ports == "" {
			ports = "-"
		}
		pid := "-"
		if c.PID > 0 {
			pid = fmt.Sprintf("%d", c.PID)
		}
		fmt.Printf("%-8s %-24s %-24s %-10s %-8s %s\n", svcName, c.ID, c.Image, c.Status, pid, ports)
	}

	return nil
}

// orderServices returns service names in dependency order (depends_on first)
// using a simple topological sort.
func orderServices(services map[string]ComposeService) []string {
	var ordered []string
	visited := make(map[string]bool)

	var visit func(name string)
	visit = func(name string) {
		if visited[name] {
			return
		}
		visited[name] = true
		svc := services[name]
		for _, dep := range svc.DependsOn {
			if _, ok := services[dep]; ok {
				visit(dep)
			}
		}
		ordered = append(ordered, name)
	}

	// Sort keys for deterministic output
	var keys []string
	for k := range services {
		keys = append(keys, k)
	}
	sort.Strings(keys)

	for _, k := range keys {
		visit(k)
	}

	return ordered
}
