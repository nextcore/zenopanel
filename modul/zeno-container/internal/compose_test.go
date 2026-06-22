package internal

import (
	"os"
	"path/filepath"
	"testing"

	"gopkg.in/yaml.v3"
)

func TestComposeEnvironmentUnmarshal(t *testing.T) {
	tests := []struct {
		name     string
		yamlData string
		expected map[string]string
	}{
		{
			name:     "Environment as map",
			yamlData: "ENV_VAR: value\nANOTHER_VAR: value2",
			expected: map[string]string{
				"ENV_VAR":     "value",
				"ANOTHER_VAR": "value2",
			},
		},
		{
			name:     "Environment as list of key=val",
			yamlData: "- ENV_VAR=value\n- ANOTHER_VAR=value2\n- EMPTY_VAR",
			expected: map[string]string{
				"ENV_VAR":     "value",
				"ANOTHER_VAR": "value2",
				"EMPTY_VAR":   "",
			},
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			var env ComposeEnvironment
			err := yaml.Unmarshal([]byte(tt.yamlData), &env)
			if err != nil {
				t.Fatalf("Failed to unmarshal YAML: %v", err)
			}

			if len(env) != len(tt.expected) {
				t.Errorf("Expected length %d, got %d", len(tt.expected), len(env))
			}

			for k, v := range tt.expected {
				if env[k] != v {
					t.Errorf("Expected %s=%s, got %s", k, v, env[k])
				}
			}
		})
	}
}

func TestComposePortsUnmarshal(t *testing.T) {
	yamlData := "- \"8080:80\"\n- 3000:3000\n- 80"

	var ports ComposePorts
	err := yaml.Unmarshal([]byte(yamlData), &ports)
	if err != nil {
		t.Fatalf("Failed to unmarshal ports: %v", err)
	}

	expected := []string{"8080:80", "3000:3000", "80"}
	if len(ports) != len(expected) {
		t.Fatalf("Expected %d ports, got %d", len(expected), len(ports))
	}

	for i, v := range expected {
		if ports[i] != v {
			t.Errorf("Expected port %d to be %s, got %s", i, v, ports[i])
		}
	}
}

func TestParseComposeFile(t *testing.T) {
	tempDir, err := os.MkdirTemp("", "compose-test-*")
	if err != nil {
		t.Fatalf("Failed to create temp dir: %v", err)
	}
	defer os.RemoveAll(tempDir)

	composeContent := `
version: "3"
services:
  web:
    image: nginx:alpine
    container_name: my-web-server
    ports:
      - "80:80"
    environment:
      - DEBUG=true
    restart: always
    healthcheck:
      test: ["CMD-SHELL", "curl -f http://localhost/ || exit 1"]
      interval: 10s
      timeout: 5s
      retries: 3
  db:
    image: postgres:15
    ports:
      - 5432:5432
    environment:
      POSTGRES_PASSWORD: secretpassword
    depends_on:
      - cache
  cache:
    image: redis:alpine
`
	filePath := filepath.Join(tempDir, "docker-compose.yml")
	err = os.WriteFile(filePath, []byte(composeContent), 0644)
	if err != nil {
		t.Fatalf("Failed to write temp compose file: %v", err)
	}

	cf, err := ParseComposeFile(filePath)
	if err != nil {
		t.Fatalf("ParseComposeFile failed: %v", err)
	}

	if cf.Version != "3" {
		t.Errorf("Expected version 3, got %s", cf.Version)
	}

	if len(cf.Services) != 3 {
		t.Fatalf("Expected 3 services, got %d", len(cf.Services))
	}

	// Verify web service
	web, exists := cf.Services["web"]
	if !exists {
		t.Fatal("web service not found")
	}
	if web.Image != "nginx:alpine" || web.ContainerName != "my-web-server" {
		t.Errorf("Unexpected web fields: %+v", web)
	}
	if web.Restart != "always" {
		t.Errorf("Expected web restart always, got %s", web.Restart)
	}
	hcConfig := web.HealthCheck.ToHealthCheckConfig()
	if hcConfig == nil {
		t.Fatal("Expected healthcheck config, got nil")
	}
	if len(hcConfig.Test) != 2 || hcConfig.Test[0] != "CMD-SHELL" || hcConfig.Test[1] != "curl -f http://localhost/ || exit 1" {
		t.Errorf("Unexpected health check test: %+v", hcConfig.Test)
	}
	if hcConfig.Interval != 10 || hcConfig.Timeout != 5 || hcConfig.Retries != 3 {
		t.Errorf("Unexpected health check config values: %+v", hcConfig)
	}

	// Verify depends_on ordering
	ordered := orderServices(cf.Services)
	if len(ordered) != 3 {
		t.Fatalf("Expected 3 ordered services, got %d", len(ordered))
	}
	// db depends on cache, so cache must be started before db
	cacheIdx, dbIdx := -1, -1
	for i, name := range ordered {
		if name == "cache" {
			cacheIdx = i
		}
		if name == "db" {
			dbIdx = i
		}
	}
	if cacheIdx > dbIdx {
		t.Errorf("Expected cache to be started before db, but cache index %d is after db index %d", cacheIdx, dbIdx)
	}
}
