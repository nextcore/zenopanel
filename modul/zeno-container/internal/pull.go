package internal

import (
	"archive/tar"
	"compress/gzip"
	"crypto/sha256"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"os"
	"path/filepath"
	"strings"
	"time"
)

// Registry URLs
const (
	dockerHubAuth     = "https://auth.docker.io/token"
	dockerHubRegistry = "https://registry-1.docker.io"
)

// Docker manifest media types
const (
	mediaTypeManifestList = "application/vnd.docker.distribution.manifest.list.v2+json"
	mediaTypeManifestV2   = "application/vnd.docker.distribution.manifest.v2+json"
	mediaTypeOCIIndex     = "application/vnd.oci.image.index.v1+json"
	mediaTypeOCIManifest  = "application/vnd.oci.image.manifest.v1+json"
)

// manifestV2 is a Docker Image Manifest V2, Schema 2.
type manifestV2 struct {
	SchemaVersion int            `json:"schemaVersion"`
	MediaType     string         `json:"mediaType"`
	Config        manifestBlob   `json:"config"`
	Layers        []manifestBlob `json:"layers"`
}

// manifestList is a Docker Manifest List (for multi-arch images).
type manifestList struct {
	SchemaVersion int                `json:"schemaVersion"`
	MediaType     string             `json:"mediaType"`
	Manifests     []manifestListItem `json:"manifests"`
}

type manifestListItem struct {
	MediaType string `json:"mediaType"`
	Size      int64  `json:"size"`
	Digest    string `json:"digest"`
	Platform  struct {
		Architecture string `json:"architecture"`
		OS           string `json:"os"`
	} `json:"platform"`
}

type manifestBlob struct {
	MediaType string `json:"mediaType"`
	Size      int64  `json:"size"`
	Digest    string `json:"digest"`
}

// imageConfig is the image configuration blob (contains Cmd, Entrypoint, Env, etc.)
type imageConfig struct {
	Config struct {
		Cmd          []string            `json:"Cmd"`
		Entrypoint   []string            `json:"Entrypoint"`
		Env          []string            `json:"Env"`
		WorkingDir   string              `json:"WorkingDir"`
		ExposedPorts map[string]struct{} `json:"ExposedPorts"`
		Labels       map[string]string   `json:"Labels"`
	} `json:"config"`
	RootFS struct {
		Type    string   `json:"type"`
		DiffIDs []string `json:"diff_ids"`
	} `json:"rootfs"`
	History []struct {
		Created    string `json:"created"`
		EmptyLayer bool   `json:"empty_layer"`
	} `json:"history"`
}

// parsedImageRef holds the parsed components of an image reference.
type parsedImageRef struct {
	Registry   string
	Repository string
	Tag        string
}

func parseImageRef(image string) parsedImageRef {
	ref := parsedImageRef{
		Registry: dockerHubRegistry,
		Tag:      "latest",
	}

	// Check for custom registry (contains a dot or colon, but not a slash at the start)
	parts := strings.SplitN(image, "/", 2)
	if len(parts) == 2 && (strings.Contains(parts[0], ".") || strings.Contains(parts[0], ":")) {
		ref.Registry = "https://" + parts[0]
		image = parts[1]
	}

	// Split repo:tag
	if idx := strings.LastIndex(image, ":"); idx != -1 {
		ref.Repository = image[:idx]
		ref.Tag = image[idx+1:]
	} else {
		ref.Repository = image
	}

	// Docker Hub uses library/ prefix for official images
	if ref.Registry == dockerHubRegistry && !strings.Contains(ref.Repository, "/") {
		ref.Repository = "library/" + ref.Repository
	}

	return ref
}

// getRegistryHost extracts the hostname from registry URL.
func getRegistryHost(registryURL string) string {
	h := registryURL
	h = strings.TrimPrefix(h, "https://")
	h = strings.TrimPrefix(h, "http://")
	return h
}

// parseWwwAuthenticate parses Www-Authenticate header to extract realm, service, and scope
func parseWwwAuthenticate(header string) map[string]string {
	params := make(map[string]string)
	if !strings.HasPrefix(strings.ToLower(header), "bearer ") {
		return params
	}
	parts := strings.Split(header[7:], ",")
	for _, p := range parts {
		kv := strings.SplitN(strings.TrimSpace(p), "=", 2)
		if len(kv) == 2 {
			k := kv[0]
			v := strings.Trim(kv[1], "\"")
			params[k] = v
		}
	}
	return params
}

// resolveRegistryToken handles standard Bearer/token authorization by executing a probe request or querying direct auth endpoint.
func resolveRegistryToken(registryHost, repository string) (string, error) {
	var authURL string
	if registryHost == "registry-1.docker.io" {
		authURL = fmt.Sprintf("%s?service=registry.docker.io&scope=repository:%s:pull", dockerHubAuth, repository)
	} else {
		// Attempt a probe request to trigger 401 challenge
		probeURL := fmt.Sprintf("https://%s/v2/%s/manifests/latest", registryHost, repository)
		client := &http.Client{Timeout: 10 * time.Second}
		req, _ := http.NewRequest("GET", probeURL, nil)
		resp, err := client.Do(req)
		if err != nil {
			// If https fails, try http (useful for local private registries)
			probeURL = fmt.Sprintf("http://%s/v2/%s/manifests/latest", registryHost, repository)
			req, _ = http.NewRequest("GET", probeURL, nil)
			resp, err = client.Do(req)
		}
		if err == nil {
			defer resp.Body.Close()
			if resp.StatusCode == http.StatusUnauthorized {
				wwwAuth := resp.Header.Get("Www-Authenticate")
				params := parseWwwAuthenticate(wwwAuth)
				realm := params["realm"]
				service := params["service"]
				scope := params["scope"]
				if realm != "" {
					authURL = realm
					var query []string
					if service != "" {
						query = append(query, "service="+service)
					}
					if scope != "" {
						query = append(query, "scope="+scope)
					} else {
						query = append(query, fmt.Sprintf("scope=repository:%s:pull", repository))
					}
					if len(query) > 0 {
						if strings.Contains(authURL, "?") {
							authURL += "&" + strings.Join(query, "&")
						} else {
							authURL += "?" + strings.Join(query, "&")
						}
					}
				}
			}
		}
	}

	if authURL == "" {
		// No token auth challenge detected. Returns empty token to fallback to Basic directly on requests.
		return "", nil
	}

	client := &http.Client{Timeout: 30 * time.Second}
	req, err := http.NewRequest("GET", authURL, nil)
	if err != nil {
		return "", err
	}

	// Attach credentials if we have them
	if username, password, ok := GetRegistryCredentials(registryHost); ok {
		req.SetBasicAuth(username, password)
	}

	resp, err := client.Do(req)
	if err != nil {
		return "", fmt.Errorf("auth request failed: %w", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		body, _ := io.ReadAll(resp.Body)
		return "", fmt.Errorf("auth request returned %d: %s", resp.StatusCode, string(body))
	}

	var result struct {
		Token       string `json:"token"`
		AccessToken string `json:"access_token"`
	}
	if err := json.NewDecoder(resp.Body).Decode(&result); err != nil {
		return "", fmt.Errorf("failed to decode auth response: %w", err)
	}

	token := result.Token
	if token == "" {
		token = result.AccessToken
	}
	return token, nil
}

// doRegistryRequest performs an authenticated request to a registry.
// Accepts both manifest V2 and manifest list.
func doRegistryRequest(url, token, registryHost string) (*http.Response, error) {
	client := &http.Client{Timeout: 60 * time.Second}
	req, err := http.NewRequest("GET", url, nil)
	if err != nil {
		return nil, err
	}
	if token != "" {
		req.Header.Set("Authorization", "Bearer "+token)
	} else if username, password, ok := GetRegistryCredentials(registryHost); ok {
		req.SetBasicAuth(username, password)
	}
	// Accept both manifest V2 and manifest list
	req.Header.Set("Accept", fmt.Sprintf("%s,%s", mediaTypeManifestV2, mediaTypeManifestList))

	resp, err := client.Do(req)
	if err != nil {
		return nil, fmt.Errorf("request to %s failed: %w", url, err)
	}
	return resp, nil
}

// doRegistryRequestManifest fetches a manifest, accepting only the V2 manifest.
func doRegistryRequestManifest(url, token, registryHost string) (*http.Response, error) {
	client := &http.Client{Timeout: 60 * time.Second}
	req, err := http.NewRequest("GET", url, nil)
	if err != nil {
		return nil, err
	}
	if token != "" {
		req.Header.Set("Authorization", "Bearer "+token)
	} else if username, password, ok := GetRegistryCredentials(registryHost); ok {
		req.SetBasicAuth(username, password)
	}
	req.Header.Set("Accept", mediaTypeManifestV2)

	resp, err := client.Do(req)
	if err != nil {
		return nil, fmt.Errorf("request to %s failed: %w", url, err)
	}
	return resp, nil
}

// resolveManifest fetches the manifest and resolves manifest lists by picking
// the linux/amd64 platform. Returns a parsed manifestV2.
func resolveManifest(registryURL, repository, tag, token, registryHost string) (*manifestV2, error) {
	manifestURL := fmt.Sprintf("%s/v2/%s/manifests/%s", registryURL, repository, tag)

	// Accept both and handle whatever the registry returns
	resp, err := doRegistryRequest(manifestURL, token, registryHost)
	if err != nil {
		return nil, err
	}
	defer resp.Body.Close()

	body, err := io.ReadAll(resp.Body)
	if err != nil {
		return nil, fmt.Errorf("failed to read manifest body: %w", err)
	}

	contentType := resp.Header.Get("Content-Type")
	fmt.Printf("  ▶ Content-Type: %s\n", contentType)

	// If it's a manifest list/index, resolve it
	if contentType == mediaTypeManifestList || contentType == mediaTypeOCIIndex {
		var list manifestList
		if err := json.Unmarshal(body, &list); err != nil {
			return nil, fmt.Errorf("failed to decode manifest list: %w", err)
		}

		fmt.Printf("  ▶ Manifest list with %d entries\n", len(list.Manifests))
		for _, m := range list.Manifests {
			fmt.Printf("    - %s/%s: %s\n", m.Platform.OS, m.Platform.Architecture, m.Digest[:20])
		}

		// Pick linux/amd64
		var selectedDigest string
		for _, m := range list.Manifests {
			if m.Platform.OS == "linux" && m.Platform.Architecture == "amd64" {
				selectedDigest = m.Digest
				break
			}
		}
		if selectedDigest == "" && len(list.Manifests) > 0 {
			selectedDigest = list.Manifests[0].Digest
		}
		if selectedDigest == "" {
			return nil, fmt.Errorf("no manifests found in manifest list")
		}

		fmt.Printf("  ▶ Resolved to manifest digest: %s\n", selectedDigest[:20])

		// Fetch the actual manifest by digest — request V2 manifest only
		manifestByDigestURL := fmt.Sprintf("%s/v2/%s/manifests/%s", registryURL, repository, selectedDigest)
		resp2, err := doRegistryRequestManifest(manifestByDigestURL, token, registryHost)
		if err != nil {
			return nil, err
		}
		defer resp2.Body.Close()

		if resp2.StatusCode != http.StatusOK {
			bodyBytes, _ := io.ReadAll(resp2.Body)
			return nil, fmt.Errorf("manifest by digest returned %d: %s", resp2.StatusCode, string(bodyBytes))
		}

		body2, err := io.ReadAll(resp2.Body)
		if err != nil {
			return nil, fmt.Errorf("failed to read resolved manifest: %w", err)
		}

		var m manifestV2
		if err := json.Unmarshal(body2, &m); err != nil {
			return nil, fmt.Errorf("failed to decode resolved manifest: %w", err)
		}
		return &m, nil
	}

	// It's a regular manifest V2
	var manifest manifestV2
	if err := json.Unmarshal(body, &manifest); err != nil {
		return nil, fmt.Errorf("failed to decode manifest: %w", err)
	}
	return &manifest, nil
}

// cacheDirName creates a safe directory name from image repository and tag.
func cacheDirName(repository, tag string) string {
	// Replace slashes and special chars to make a flat directory name
	s := repository + "_" + tag
	s = strings.NewReplacer("/", "_", ":", "_").Replace(s)
	return s
}

// PullImage pulls an OCI/Docker image from a registry and caches its layers.
// Returns the command (entrypoint+cmd) from the image config.
func PullImage(image string, dataDir string) ([]string, error) {
	ref := parseImageRef(image)
	fmt.Printf("  ▶ Registry: %s\n", ref.Registry)
	fmt.Printf("  ▶ Repository: %s\n", ref.Repository)
	fmt.Printf("  ▶ Tag: %s\n", ref.Tag)

	// 1. Resolve registry host and get auth token
	registryHost := getRegistryHost(ref.Registry)
	token, err := resolveRegistryToken(registryHost, ref.Repository)
	if err != nil {
		return nil, fmt.Errorf("failed to get auth token: %w", err)
	}

	cacheDir := ImageCacheDir(dataDir) + "/" + cacheDirName(ref.Repository, ref.Tag)
	if err := os.MkdirAll(cacheDir, 0755); err != nil {
		return nil, fmt.Errorf("failed to create cache dir: %w", err)
	}

	layersDir := ImageCacheDir(dataDir) + "/layers"
	if err := os.MkdirAll(layersDir, 0755); err != nil {
		return nil, fmt.Errorf("failed to create layers cache dir: %w", err)
	}

	// 2. Resolve manifest (handles manifest lists for multi-arch images)
	fmt.Printf("  ▶ Fetching manifest...\n")
	manifest, err := resolveManifest(ref.Registry, ref.Repository, ref.Tag, token, registryHost)
	if err != nil {
		return nil, fmt.Errorf("manifest resolution failed: %w", err)
	}

	fmt.Printf("  ▶ Layers: %d\n", len(manifest.Layers))

	// 3. Fetch image config (to get Cmd, Entrypoint, Env, etc.)
	configURL := fmt.Sprintf("%s/v2/%s/blobs/%s", ref.Registry, ref.Repository, manifest.Config.Digest)
	fmt.Printf("  ▶ Fetching image config...\n")

	resp2, err := doRegistryRequest(configURL, token, registryHost)
	if err != nil {
		return nil, fmt.Errorf("config request failed: %w", err)
	}
	defer resp2.Body.Close()

	if resp2.StatusCode != http.StatusOK {
		return nil, fmt.Errorf("config request returned %d", resp2.StatusCode)
	}

	var imgCfg imageConfig
	if err := json.NewDecoder(resp2.Body).Decode(&imgCfg); err != nil {
		return nil, fmt.Errorf("failed to decode image config: %w", err)
	}

	// Build command from Entrypoint + Cmd
	cmd := append(imgCfg.Config.Entrypoint, imgCfg.Config.Cmd...)

	// 4. Download and cache each layer separately
	var layerDigests []string
	for i, layer := range manifest.Layers {
		layerDigest := strings.TrimPrefix(layer.Digest, "sha256:")
		layerDigests = append(layerDigests, layerDigest)

		layerCacheDir := layersDir + "/" + layerDigest
		layerRootfsDir := layerCacheDir + "/rootfs"
		cacheFile := layerCacheDir + "/" + layerDigest + ".tar.gz"

		if err := os.MkdirAll(layerCacheDir, 0755); err != nil {
			return nil, fmt.Errorf("failed to create layer cache dir: %w", err)
		}

		// Check if already extracted
		if _, err := os.Stat(layerRootfsDir); os.IsNotExist(err) {
			if _, err := os.Stat(cacheFile); os.IsNotExist(err) {
				fmt.Printf("  ▶ Downloading layer %d/%d (%s)...\n", i+1, len(manifest.Layers), layer.Digest[:20])

				blobURL := fmt.Sprintf("%s/v2/%s/blobs/%s", ref.Registry, ref.Repository, layer.Digest)
				resp3, err := doRegistryRequest(blobURL, token, registryHost)
				if err != nil {
					return nil, fmt.Errorf("layer %d download failed: %w", i+1, err)
				}
				defer resp3.Body.Close()

				if resp3.StatusCode != http.StatusOK {
					return nil, fmt.Errorf("layer %d request returned %d", i+1, resp3.StatusCode)
				}

				// Verify digest while writing to cache
				hash := sha256.New()
				tee := io.TeeReader(resp3.Body, hash)

				f, err := os.Create(cacheFile)
				if err != nil {
					return nil, fmt.Errorf("failed to create cache file: %w", err)
				}

				written, err := io.Copy(f, tee)
				f.Close()
				if err != nil {
					return nil, fmt.Errorf("failed to download layer %d: %w", i+1, err)
				}

				computedDigest := fmt.Sprintf("sha256:%x", hash.Sum(nil))
				if computedDigest != layer.Digest {
					os.Remove(cacheFile)
					return nil, fmt.Errorf("layer %d digest mismatch: expected %s, got %s", i+1, layer.Digest, computedDigest)
				}

				fmt.Printf("    ✓ Layer %d: %d bytes, verified\n", i+1, written)
			} else {
				fmt.Printf("  ▶ Using cached archive for layer %d/%d\n", i+1, len(manifest.Layers))
			}

			// Extract layer to layerRootfsDir
			fmt.Printf("  ▶ Extracting layer %d/%d...\n", i+1, len(manifest.Layers))
			if err := os.MkdirAll(layerRootfsDir, 0755); err != nil {
				return nil, fmt.Errorf("failed to create layer rootfs dir: %w", err)
			}
			if err := extractTarGz(cacheFile, layerRootfsDir); err != nil {
				return nil, fmt.Errorf("failed to extract layer %d: %w", i+1, err)
			}
		} else {
			fmt.Printf("  ▶ Layer %d/%d already extracted\n", i+1, len(manifest.Layers))
		}
	}

	// Save layers order list
	layersData, _ := json.Marshal(layerDigests)
	os.WriteFile(cacheDir+"/layers.json", layersData, 0644)

	// Save image config for later use
	cfgData, _ := json.MarshalIndent(imgCfg, "", "  ")
	os.WriteFile(cacheDir+"/image-config.json", cfgData, 0644)

	fmt.Printf("  ✓ Image pulled successfully!\n")
	return cmd, nil
}

// extractTarGz extracts a .tar.gz file to the specified directory.
func extractTarGz(src, dest string) error {
	f, err := os.Open(src)
	if err != nil {
		return err
	}
	defer f.Close()

	gzr, err := gzip.NewReader(f)
	if err != nil {
		return err
	}
	defer gzr.Close()

	tr := tar.NewReader(gzr)
	for {
		header, err := tr.Next()
		if err == io.EOF {
			break
		}
		if err != nil {
			return err
		}

		// Sanitize path
		target := filepath.Join(dest, header.Name)
		if !strings.HasPrefix(target, filepath.Clean(dest)+string(os.PathSeparator)) {
			return fmt.Errorf("invalid tar path: %s", header.Name)
		}

		mode := header.FileInfo().Mode()

		switch header.Typeflag {
		case tar.TypeDir:
			if err := os.MkdirAll(target, mode); err != nil {
				return err
			}
		case tar.TypeReg, tar.TypeRegA:
			if err := os.MkdirAll(filepath.Dir(target), 0755); err != nil {
				return err
			}
			outFile, err := os.OpenFile(target, os.O_CREATE|os.O_WRONLY|os.O_TRUNC, mode)
			if err != nil {
				return err
			}
			if _, err := io.Copy(outFile, tr); err != nil {
				outFile.Close()
				return err
			}
			outFile.Close()
		case tar.TypeSymlink:
			if err := os.MkdirAll(filepath.Dir(target), 0755); err != nil {
				return err
			}
			os.Remove(target)
			if err := os.Symlink(header.Linkname, target); err != nil {
				return err
			}
		case tar.TypeLink:
			if err := os.MkdirAll(filepath.Dir(target), 0755); err != nil {
				return err
			}
			linkTarget := filepath.Join(dest, header.Linkname)
			os.Remove(target)
			if err := os.Link(linkTarget, target); err != nil {
				return err
			}
		}
	}

	return nil
}

// ListCachedImages lists all cached images in the data directory.
func ListCachedImages(dataDir string) ([]string, error) {
	cacheDir := ImageCacheDir(dataDir)
	entries, err := os.ReadDir(cacheDir)
	if err != nil {
		if os.IsNotExist(err) {
			return nil, nil
		}
		return nil, err
	}

	var images []string
	for _, e := range entries {
		if e.IsDir() && e.Name() != "layers" {
			name := strings.ReplaceAll(e.Name(), "_", "/")
			if idx := strings.LastIndex(name, "/"); idx != -1 {
				name = name[:idx] + ":" + name[idx+1:]
			}
			images = append(images, name)
		}
	}
	return images, nil
}

// RemoveCachedImage removes a cached image from the data directory.
func RemoveCachedImage(image string, dataDir string) error {
	ref := parseImageRef(image)
	cacheDir := ImageCacheDir(dataDir) + "/" + cacheDirName(ref.Repository, ref.Tag)
	return os.RemoveAll(cacheDir)
}
