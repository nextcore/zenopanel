package internal

import (
	"encoding/base64"
	"encoding/json"
	"os"
	"path/filepath"
	"strings"
)

// RegistryConfig matches the structure of Docker's config.json
type RegistryConfig struct {
	Auths map[string]RegistryAuth `json:"auths"`
}

type RegistryAuth struct {
	Auth string `json:"auth"` // base64(username:password)
}

// GetConfigPath returns the path to ~/.zeno-container/config.json
func GetConfigPath() (string, error) {
	home, err := os.UserHomeDir()
	if err != nil {
		return "", err
	}
	return filepath.Join(home, ".zeno-container", "config.json"), nil
}

// LoadRegistryConfig reads the config file from ~/.zeno-container/config.json
func LoadRegistryConfig() (*RegistryConfig, error) {
	path, err := GetConfigPath()
	if err != nil {
		return nil, err
	}

	data, err := os.ReadFile(path)
	if err != nil {
		if os.IsNotExist(err) {
			return &RegistryConfig{Auths: make(map[string]RegistryAuth)}, nil
		}
		return nil, err
	}

	var config RegistryConfig
	if err := json.Unmarshal(data, &config); err != nil {
		return nil, err
	}
	if config.Auths == nil {
		config.Auths = make(map[string]RegistryAuth)
	}
	return &config, nil
}

// SaveRegistryConfig writes the config file to ~/.zeno-container/config.json
func SaveRegistryConfig(config *RegistryConfig) error {
	path, err := GetConfigPath()
	if err != nil {
		return err
	}

	dir := filepath.Dir(path)
	if err := os.MkdirAll(dir, 0700); err != nil {
		return err
	}

	data, err := json.MarshalIndent(config, "", "  ")
	if err != nil {
		return err
	}

	return os.WriteFile(path, data, 0600)
}

// GetRegistryCredentials retrieves credentials for a given registry host.
func GetRegistryCredentials(registry string) (string, string, bool) {
	config, err := LoadRegistryConfig()
	if err != nil {
		return "", "", false
	}

	// Normalize registry name
	registry = normalizeRegistryHost(registry)

	// Try direct match
	if authEntry, ok := config.Auths[registry]; ok {
		return decodeAuth(authEntry.Auth)
	}

	// Try with https:// prefix or without
	var alternative string
	if strings.HasPrefix(registry, "https://") {
		alternative = strings.TrimPrefix(registry, "https://")
	} else {
		alternative = "https://" + registry
	}
	if authEntry, ok := config.Auths[alternative]; ok {
		return decodeAuth(authEntry.Auth)
	}

	// Docker Hub specific fallback keys
	if registry == "registry-1.docker.io" || registry == "https://registry-1.docker.io" {
		for _, key := range []string{"https://index.docker.io/v1/", "index.docker.io", "docker.io"} {
			if authEntry, ok := config.Auths[key]; ok {
				return decodeAuth(authEntry.Auth)
			}
		}
	}

	return "", "", false
}

// SaveRegistryCredentials stores the credentials for a registry host.
func SaveRegistryCredentials(registry, username, password string) error {
	config, err := LoadRegistryConfig()
	if err != nil {
		return err
	}

	registry = normalizeRegistryHost(registry)

	pair := username + ":" + password
	encoded := base64.StdEncoding.EncodeToString([]byte(pair))

	config.Auths[registry] = RegistryAuth{Auth: encoded}

	return SaveRegistryConfig(config)
}

func normalizeRegistryHost(host string) string {
	host = strings.TrimSuffix(host, "/")
	return host
}

func decodeAuth(authStr string) (string, string, bool) {
	decoded, err := base64.StdEncoding.DecodeString(authStr)
	if err != nil {
		return "", "", false
	}
	parts := strings.SplitN(string(decoded), ":", 2)
	if len(parts) != 2 {
		return "", "", false
	}
	return parts[0], parts[1], true
}
