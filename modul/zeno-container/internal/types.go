package internal

import "time"

// ContainerState represents a container managed by zeno-container.
type ContainerState struct {
	ID          string            `json:"id"`
	Image       string            `json:"image"`
	Status      string            `json:"status"` // created, running, stopped, failed
	PID         int               `json:"pid"`
	CreatedAt   string            `json:"created_at"`
	ExitedAt    string            `json:"exited_at,omitempty"`
	ExitCode    int               `json:"exit_code,omitempty"`
	Cmd         []string          `json:"cmd"`
	LogPath     string            `json:"log_path,omitempty"`
	Ports         []string           `json:"ports,omitempty"` // format: "hostPort:containerPort"
	Env           map[string]string  `json:"env,omitempty"`
	Mounts        []string           `json:"mounts,omitempty"` // format: "hostPath:containerPath"
	Cwd           string             `json:"cwd,omitempty"`
	HostNetwork   bool               `json:"host_network,omitempty"`
	ProxyPID      int                `json:"proxy_pid,omitempty"`
	RestartPolicy string             `json:"restart_policy,omitempty"` // always, on-failure, unless-stopped, no
	DesiredStatus string             `json:"desired_status,omitempty"` // running, stopped
	MemoryLimit   int64              `json:"memory_limit,omitempty"`   // in bytes
	CPULimit      float64            `json:"cpu_limit,omitempty"`      // fractional cores
	OOMScoreAdj   *int               `json:"oom_score_adj,omitempty"`
	ReadOnly      bool               `json:"read_only,omitempty"`
	HealthCheck   *HealthCheckConfig `json:"health_check,omitempty"`
	HealthStatus  string             `json:"health_status,omitempty"` // healthy, unhealthy, starting
	Network       string             `json:"network,omitempty"`
}

// NetworkConfig represents a persistent custom bridge network.
type NetworkConfig struct {
	ID      string `json:"id"`      // e.g. "zenobr21"
	Name    string `json:"name"`    // e.g. "my-net"
	Driver  string `json:"driver"`  // e.g. "bridge"
	Subnet  string `json:"subnet"`  // e.g. "172.21.0.0/16"
	Gateway string `json:"gateway"` // e.g. "172.21.0.1"
}

type HealthCheckConfig struct {
	Test     []string `json:"test,omitempty"`     // e.g. ["CMD-SHELL", "curl -f ..."] or ["TCP", "80"]
	Interval int      `json:"interval,omitempty"` // in seconds
	Timeout  int      `json:"timeout,omitempty"`  // in seconds
	Retries  int      `json:"retries,omitempty"`
}

const (
	StatusCreated = "created"
	StatusRunning = "running"
	StatusStopped = "stopped"
	StatusFailed  = "failed"
)

// DefaultDataDir is the default data directory for container state and cached images.
const DefaultDataDir = "/var/lib/zeno-container"

// RuncRoot is the runc state directory root.
const RuncRoot = DefaultDataDir + "/runc"

// DefaultRuncPath is the default runc binary path.
const DefaultRuncPath = "runc"

// EmbeddedRuncPath is the embed-relative path to the runc binary.
const EmbeddedRuncPath = "runtimedeps/runc-linux-amd64"

// NewContainerState creates a new ContainerState with default values.
func NewContainerState(id, image string, cmd []string) *ContainerState {
	return &ContainerState{
		ID:        id,
		Image:     image,
		Status:    StatusCreated,
		CreatedAt: time.Now().UTC().Format(time.RFC3339),
		Cmd:       cmd,
		Env:       make(map[string]string),
	}
}

// ImageCacheDir returns the directory where image layers are cached.
func ImageCacheDir(dataDir string) string {
	return dataDir + "/images"
}

// ContainerDir returns the directory for a specific container.
func ContainerDir(dataDir, containerID string) string {
	return dataDir + "/containers/" + containerID
}

// BundleDir returns the OCI bundle directory for a container.
func BundleDir(dataDir, containerID string) string {
	return ContainerDir(dataDir, containerID) + "/bundle"
}

// RootfsDir returns the root filesystem directory for a container.
func RootfsDir(dataDir, containerID string) string {
	return BundleDir(dataDir, containerID) + "/rootfs"
}

// StateFile returns the path to the container state JSON file.
func StateFile(dataDir, containerID string) string {
	return ContainerDir(dataDir, containerID) + "/state.json"
}

// ContainerLogPath returns the path to the container's console log file.
func ContainerLogPath(dataDir, containerID string) string {
	return ContainerDir(dataDir, containerID) + "/console.log"
}
