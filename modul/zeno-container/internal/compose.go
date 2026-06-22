package internal

import (
	"fmt"
	"os"
	"sort"
	"strconv"
	"strings"
	"time"

	"gopkg.in/yaml.v3"
)

// ComposeEnvironment supports unmarshaling environment variables as a map or list
type ComposeEnvironment map[string]string

func (ce *ComposeEnvironment) UnmarshalYAML(value *yaml.Node) error {
	*ce = make(map[string]string)
	var m map[string]string
	if err := value.Decode(&m); err == nil {
		*ce = m
		return nil
	}
	var s []string
	if err := value.Decode(&s); err == nil {
		for _, item := range s {
			parts := strings.SplitN(item, "=", 2)
			if len(parts) == 2 {
				(*ce)[parts[0]] = parts[1]
			} else if len(parts) == 1 {
				(*ce)[parts[0]] = ""
			}
		}
		return nil
	}
	return fmt.Errorf("failed to unmarshal environment: must be a map or a list of strings")
}

// ComposePorts supports unmarshaling ports as integers or strings
type ComposePorts []string

func (cp *ComposePorts) UnmarshalYAML(value *yaml.Node) error {
	*cp = make(ComposePorts, 0)
	var raw []interface{}
	if err := value.Decode(&raw); err != nil {
		return err
	}
	for _, r := range raw {
		switch v := r.(type) {
		case string:
			*cp = append(*cp, v)
		case int:
			*cp = append(*cp, strconv.Itoa(v))
		case int64:
			*cp = append(*cp, strconv.FormatInt(v, 10))
		default:
			*cp = append(*cp, fmt.Sprintf("%v", v))
		}
	}
	return nil
}

// ComposeHealthCheck represents health check configuration in docker-compose.yml
type ComposeHealthCheck struct {
	Test     yaml.Node `yaml:"test"`
	Interval string    `yaml:"interval"`
	Timeout  string    `yaml:"timeout"`
	Retries  int       `yaml:"retries"`
}

func parseDurationSeconds(dStr string) int {
	if dStr == "" {
		return 0
	}
	dStr = strings.TrimSpace(dStr)
	dur, err := time.ParseDuration(dStr)
	if err == nil {
		return int(dur.Seconds())
	}
	val, err := strconv.Atoi(dStr)
	if err == nil {
		return val
	}
	if strings.HasSuffix(dStr, "s") {
		val, _ = strconv.Atoi(strings.TrimSuffix(dStr, "s"))
		return val
	}
	if strings.HasSuffix(dStr, "m") {
		val, _ = strconv.Atoi(strings.TrimSuffix(dStr, "m"))
		return val * 60
	}
	return 0
}

func (chc *ComposeHealthCheck) ToHealthCheckConfig() *HealthCheckConfig {
	if chc == nil || chc.Test.Kind == 0 {
		return nil
	}

	var test []string
	if chc.Test.Kind == yaml.ScalarNode {
		test = []string{"CMD-SHELL", chc.Test.Value}
	} else if chc.Test.Kind == yaml.SequenceNode {
		for _, n := range chc.Test.Content {
			test = append(test, n.Value)
		}
	}

	if len(test) == 0 {
		return nil
	}

	interval := parseDurationSeconds(chc.Interval)
	timeout := parseDurationSeconds(chc.Timeout)
	retries := chc.Retries

	if interval == 0 {
		interval = 30
	}
	if timeout == 0 {
		timeout = 5
	}
	if retries == 0 {
		retries = 3
	}

	return &HealthCheckConfig{
		Test:     test,
		Interval: interval,
		Timeout:  timeout,
		Retries:  retries,
	}
}

// ComposeService represents a service in docker-compose.yml
type ComposeService struct {
	Image         string             `yaml:"image"`
	ContainerName string             `yaml:"container_name"`
	Ports         ComposePorts       `yaml:"ports"`
	Environment   ComposeEnvironment `yaml:"environment"`
	Volumes       []string           `yaml:"volumes"`
	Command       string             `yaml:"command"`
	DependsOn     []string           `yaml:"depends_on"`
	Networks      []string           `yaml:"networks"`
	Restart       string             `yaml:"restart"`
	HealthCheck   ComposeHealthCheck `yaml:"healthcheck"`
	MemLimit      string             `yaml:"mem_limit"`
	CPUs          float64            `yaml:"cpus"`
	OomScoreAdj   *int               `yaml:"oom_score_adj"`
	ReadOnly      bool               `yaml:"read_only"`
}

func parseMemoryBytes(mStr string) int64 {
	if mStr == "" {
		return 0
	}
	mStr = strings.ToLower(strings.TrimSpace(mStr))
	var unit int64 = 1
	if strings.HasSuffix(mStr, "b") {
		mStr = strings.TrimSuffix(mStr, "b")
	}
	if strings.HasSuffix(mStr, "k") {
		unit = 1024
		mStr = strings.TrimSuffix(mStr, "k")
	} else if strings.HasSuffix(mStr, "m") {
		unit = 1024 * 1024
		mStr = strings.TrimSuffix(mStr, "m")
	} else if strings.HasSuffix(mStr, "g") {
		unit = 1024 * 1024 * 1024
		mStr = strings.TrimSuffix(mStr, "g")
	}
	val, err := strconv.ParseInt(mStr, 10, 64)
	if err != nil {
		return 0
	}
	return val * unit
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

		env := map[string]string(svc.Environment)
		if env == nil {
			env = make(map[string]string)
		}

		volumes := svc.Volumes
		if volumes == nil {
			volumes = []string{}
		}

		ports := []string(svc.Ports)
		if ports == nil {
			ports = []string{}
		}

		// 3.5 If container already exists, stop and delete it to make compose up idempotent
		if _, err := os.Stat(StateFile(cm.DataDir, containerName)); err == nil {
			fmt.Printf("  ▶ Container '%s' already exists. Stopping and removing it first...\n", containerName)
			_ = cm.ContainerStop(containerName)
			_ = cm.ContainerDelete(containerName)
		}

		// 4. Create container (using bridge networking by default for compose)
		fmt.Printf("  ▶ Creating container '%s'...\n", containerName)
		restartPolicy := svc.Restart
		if restartPolicy == "" {
			restartPolicy = "no"
		}
		hcConfig := svc.HealthCheck.ToHealthCheckConfig()
		memLimit := parseMemoryBytes(svc.MemLimit)
		cpuLimit := svc.CPUs
		if err := cm.ContainerCreate(containerName, svc.Image, cmdArgs, env, "", volumes, ports, false, restartPolicy, hcConfig, memLimit, cpuLimit, svc.OomScoreAdj, svc.ReadOnly); err != nil {
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
