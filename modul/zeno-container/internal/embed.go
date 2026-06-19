package internal

import (
	"embed"
	"os"
	"os/exec"
	"path/filepath"
	"runtime"
)

//go:embed runtimedeps/runc-linux-amd64
var runcEmbedFS embed.FS

// EnsureRuncBin returns the path to a runc binary, preferring (in order):
// 1. $ZENO_CONTAINER_RUNC environment variable
// 2. runc found in PATH
// 3. Extracted embedded runc binary
func EnsureRuncBin() string {
	// 1. Check env var
	if v := os.Getenv("ZENO_CONTAINER_RUNC"); v != "" {
		return v
	}

	// 2. Check if runc is in PATH
	if _, err := exec.LookPath("runc"); err == nil {
		return "runc"
	}

	// 3. Extract embedded runc
	home, err := os.UserHomeDir()
	if err != nil {
		home = "/root"
	}
	destDir := filepath.Join(home, ".zeno-container", "bin")
	os.MkdirAll(destDir, 0755)

	arch := runtime.GOARCH
	embedPath := "runtimedeps/runc-linux-" + arch

	data, err := runcEmbedFS.ReadFile(embedPath)
	if err != nil {
		// Fallback: hope runc is on PATH or the caller handles it
		return "runc"
	}

	destPath := filepath.Join(destDir, "runc")
	if err := os.WriteFile(destPath, data, 0755); err != nil {
		return "runc"
	}
	return destPath
}
