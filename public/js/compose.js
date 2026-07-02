import { getCSRFToken } from "./utils.js";
import { showToast } from "./toast.js";

// ─── Docker Compose State & Functions ─────────────────────────────────

let activeProject = "default";
let activeFile = "docker-compose.yml";

function composeExec(action) {
  const yaml = document.getElementById("compose-yaml-input").value.trim();
  const projectName = activeProject || "default";

  const resultDiv = document.getElementById("compose-result");
  if (resultDiv) {
    resultDiv.style.display = "block";
    resultDiv.textContent = `Running compose ${action} for project ${projectName}...\n`;
  }

  const csrf = getCSRFToken();

  fetch("/api/containers/compose", {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      "X-CSRF-Token": csrf,
    },
    body: JSON.stringify({ action, yaml, project_name: projectName, file: activeFile }),
  })
    .then((res) => res.json())
    .then((res) => {
      if (res.success) {
        if (resultDiv) {
          resultDiv.textContent = res.output || "Done.\n";
          triggerAutoScroll();
        }
        showToast("success", `Compose ${action} completed`);
        loadComposeProjects();
        import("./containers.js").then((m) => m.loadContainers());
      } else {
        if (resultDiv) {
          resultDiv.textContent = res.error || "Failed.\n";
          triggerAutoScroll();
        }
        showToast("error", res.message || "Compose failed");
        loadComposeProjects();
      }
    })
    .catch((err) => {
      if (resultDiv) {
        resultDiv.textContent = "Network error.\n";
      }
      showToast("error", "Network error");
    });
}

function triggerAutoScroll() {
  const resultDiv = document.getElementById("compose-result");
  const autoScroll = document.getElementById("compose-console-autoscroll");
  if (resultDiv && autoScroll && autoScroll.checked) {
    resultDiv.scrollTop = resultDiv.scrollHeight;
  }
}

export function composeUp() {
  composeExec("up");
}

export function composeDown() {
  composeExec("down");
}

export function composePs() {
  composeExec("ps");
}

export function loadComposeYaml(projectName, fileName) {
  const name = projectName || activeProject || "default";
  const file = fileName || activeFile || "docker-compose.yml";
  
  fetch(`/api/containers/compose/yaml?project_name=${encodeURIComponent(name)}&file=${encodeURIComponent(file)}`)
    .then((res) => res.json())
    .then((res) => {
      const textarea = document.getElementById("compose-yaml-input");
      if (textarea) {
        textarea.value = res.yaml || "";
      }
    })
    .catch((err) => console.error("Error loading compose YAML:", err));
}

// ─── Project Explorer Functions ───────────────────────────────────────

export function loadComposeProjects() {
  const projectListDiv = document.getElementById("compose-project-list");
  if (!projectListDiv) return;

  Promise.all([
    fetch("/api/containers/list").then(res => res.json()).catch(() => ({ data: [] })),
    fetch("/api/containers/compose/projects").then(res => res.json()).catch(() => ({ data: [] }))
  ])
    .then(([containersRes, projectsRes]) => {
      const activeContainers = Array.isArray(containersRes.data) ? containersRes.data : [];
      
      if (!projectsRes.data || !Array.isArray(projectsRes.data)) {
        projectListDiv.innerHTML = `<div style="padding:10px; text-align:center; color:var(--text-muted); font-size:0.8rem;">No projects found</div>`;
        return;
      }

      // Filter folders only
      const projects = projectsRes.data.filter(item => item.is_dir);

      if (projects.length === 0) {
        projectListDiv.innerHTML = `<div style="padding:10px; text-align:center; color:var(--text-muted); font-size:0.8rem;">No projects found</div>`;
        return;
      }

      projectListDiv.innerHTML = "";
      projects.forEach(project => {
        const name = project.name;

        // Check if any container belongs to this compose project and is running
        const isRunning = activeContainers.some(c => 
          (c.id.startsWith(name + "_") || c.name?.startsWith(name + "_")) && 
          (c.state === "running" || c.status?.toLowerCase().includes("up"))
        );

        const item = document.createElement("div");
        item.className = `project-item ${name === activeProject ? "active" : ""}`;
        item.style.padding = "8px 12px";
        item.style.borderRadius = "6px";
        item.style.cursor = "pointer";
        item.style.fontSize = "0.85rem";
        item.style.display = "flex";
        item.style.alignItems = "center";
        item.style.justifyContent = "space-between";
        item.style.color = name === activeProject ? "#fff" : "var(--text-muted)";
        item.style.background = name === activeProject ? "rgba(59, 130, 246, 0.15)" : "transparent";
        item.style.border = name === activeProject ? "1px solid rgba(59, 130, 246, 0.25)" : "1px solid transparent";
        item.style.transition = "all 0.2s ease";

        // Hover styling
        item.addEventListener("mouseenter", () => {
          if (name !== activeProject) {
            item.style.background = "rgba(255, 255, 255, 0.03)";
            item.style.color = "var(--text-main)";
          }
        });
        item.addEventListener("mouseleave", () => {
          if (name !== activeProject) {
            item.style.background = "transparent";
            item.style.color = "var(--text-muted)";
          }
        });

        item.onclick = () => selectComposeProject(name);

        const dotColor = isRunning ? "var(--success)" : "rgba(255, 255, 255, 0.15)";
        const dotTitle = isRunning ? "Containers running" : "Containers stopped";

        item.innerHTML = `
          <div style="display:flex; align-items:center; gap:8px; overflow:hidden;">
            <i class="fa-solid fa-folder" style="color: ${name === activeProject ? "var(--accent-primary)" : "var(--warning)"}; font-size: 0.9rem; flex-shrink:0;"></i>
            <span style="font-weight: ${name === activeProject ? "500" : "normal"}; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;">${name}</span>
          </div>
          <span style="width: 8px; height: 8px; border-radius: 50%; background: ${dotColor}; display: inline-block; flex-shrink: 0; margin-left: 6px; box-shadow: ${isRunning ? "0 0 8px var(--success)" : "none"};" title="${dotTitle}"></span>
        `;
        projectListDiv.appendChild(item);
      });

      // Synchronize input elements
      const projectInput = document.getElementById("compose-project-input");
      if (projectInput) {
        projectInput.value = activeProject;
      }
      const displaySpan = document.getElementById("active-project-name-display");
      if (displaySpan) {
        displaySpan.textContent = activeProject;
      }
    })
    .catch((err) => {
      console.error("Error loading compose projects:", err);
      projectListDiv.innerHTML = `<div style="padding:10px; text-align:center; color:var(--danger); font-size:0.8rem;">Error loading projects</div>`;
    });
}

export function selectComposeProject(name) {
  activeProject = name;
  loadComposeProjects(); // Re-render lists to update active highlights
  loadComposeYaml(name, activeFile);
}

export function loadDynamicDockerTags(repo, selectElementId, defaultTags, filterRegex) {
  const select = document.getElementById(selectElementId);
  if (!select) return;

  select.innerHTML = '<option value="">Loading tags from Docker Hub...</option>';
  select.disabled = true;

  fetch(`/api/containers/docker-tags?repo=${encodeURIComponent(repo)}`)
    .then(res => res.json())
    .then(res => {
      select.disabled = false;
      select.innerHTML = "";
      
      let tags = [];
      if (res.success && res.data && Array.isArray(res.data) && res.data.length > 0) {
        tags = res.data;
        if (filterRegex) {
          tags = tags.filter(tag => filterRegex.test(tag));
        }
      }

      // If no valid tags found, fallback to defaults
      if (tags.length === 0) {
        tags = defaultTags;
      } else {
        // Sort tags in reverse numerical order
        tags.sort((a, b) => b.localeCompare(a, undefined, { numeric: true, sensitivity: 'base' }));
      }

      tags.forEach(tag => {
        const opt = document.createElement("option");
        opt.value = tag;
        opt.textContent = tag;
        if (defaultTags.includes(tag) && tag === defaultTags[0]) {
          opt.selected = true;
        }
        select.appendChild(opt);
      });
    })
    .catch((err) => {
      console.error("Failed to load dynamic tags, falling back to defaults:", err);
      select.disabled = false;
      select.innerHTML = "";
      defaultTags.forEach(tag => {
        const opt = document.createElement("option");
        opt.value = tag;
        opt.textContent = tag;
        select.appendChild(opt);
      });
    });
}

function getTemplateTipHtml(template) {
  let tip = "";
  if (template === "node") {
    tip = `Upload file kode Node.js Anda ke <strong>/var/lib/zeno-container/volumes/[nama_proyek]_app</strong>.<br>Pastikan ada file entrypoint <strong>server.js</strong> dan berkas dependencies <strong>package.json</strong> di folder tersebut.`;
  } else if (template === "laravel") {
    tip = `Upload seluruh kode framework Laravel Anda ke <strong>/var/lib/zeno-container/volumes/[nama_proyek]_app</strong>.<br>Zeno Box akan melayani aplikasi Anda menggunakan <strong>FrankenPHP secara asinkron</strong> pada port 8000.`;
  } else if (template === "dotnet") {
    tip = `Publish aplikasi Anda terlebih dahulu, lalu upload hasilnya ke <strong>/var/lib/zeno-container/volumes/[nama_proyek]_app</strong>.<br>Ubah nama DLL target di file YAML setelah proyek dibuat (default: <code>app-name.dll</code>).`;
  } else if (template === "python") {
    tip = `Upload berkas Python Anda ke <strong>/var/lib/zeno-container/volumes/[nama_proyek]_app</strong>.<br>Pastikan ada file <strong>main.py</strong> (berisi objek ASGI <code>app = FastAPI()</code>) dan <strong>requirements.txt</strong>.`;
  } else if (template === "go") {
    tip = `Upload binary Go terkompilasi Anda ke <strong>/var/lib/zeno-container/volumes/[nama_proyek]_app</strong>.<br>Beri nama berkas binary Anda <strong>main-binary</strong> agar kontainer dapat mengeksekusinya secara otomatis.`;
  } else if (template === "ruby") {
    tip = `Upload berkas Ruby on Rails Anda ke <strong>/var/lib/zeno-container/volumes/[nama_proyek]_app</strong>.<br>Kontainer akan menjalankan <code>rails server -b 0.0.0.0</code> secara default.`;
  } else if (template === "java") {
    tip = `Upload berkas compile package JAR Anda ke <strong>/var/lib/zeno-container/volumes/[nama_proyek]_app</strong>.<br>Beri nama berkas JAR tersebut <strong>app.jar</strong> agar kontainer Java Spring Boot dapat menjalankannya.`;
  } else if (template === "rust") {
    tip = `Upload binary Rust terkompilasi (target release-musl) ke <strong>/var/lib/zeno-container/volumes/[nama_proyek]_app</strong>.<br>Beri nama berkas binary tersebut <strong>release-binary</strong>.`;
  }

  if (!tip) return "";

  return `
    <div style="margin-top:15px; padding:12px; border-radius:6px; border:1px solid rgba(59,130,246,0.2); background:rgba(59,130,246,0.04); font-size:0.78rem; line-height:1.5; color:var(--text-muted);">
      <div style="display:flex; align-items:center; gap:6px; color:var(--accent-primary); font-weight:600; margin-bottom:6px;">
        <i class="fa-solid fa-circle-info"></i> Petunjuk Deployment
      </div>
      <div>${tip}</div>
    </div>
  `;
}

export function initComposeTemplateOptionsListener() {
  const select = document.getElementById("new-compose-project-template");
  const optionsDiv = document.getElementById("compose-template-options");
  if (!select || !optionsDiv) return;

  select.addEventListener("change", (e) => {
    const template = e.target.value;
    optionsDiv.innerHTML = "";

    if (template === "custom") {
      optionsDiv.style.display = "none";
      return;
    }

    optionsDiv.style.display = "block";
    let html = "";

    if (template === "laravel") {
      html = `
        <div class="form-group" style="margin-bottom:12px;">
            <label style="display:block; margin-bottom:4px; font-weight:500; font-size:0.8rem; color:var(--text-muted);">Web Server / Engine</label>
            <select id="compose-opt-laravel-server" style="width:100%; padding:8px; border-radius:4px; border:1px solid var(--card-border); background:rgba(15,23,42,0.9); color:var(--text-main); font-size:0.8rem; outline:none; cursor:pointer;">
                <option value="frankenphp">FrankenPHP (Standalone, Asynchronous)</option>
            </select>
        </div>
        <div class="form-group" style="margin-bottom:12px;">
            <label style="display:block; margin-bottom:4px; font-weight:500; font-size:0.8rem; color:var(--text-muted);">PHP Version</label>
            <select id="compose-opt-laravel-phpver" style="width:100%; padding:8px; border-radius:4px; border:1px solid var(--card-border); background:rgba(15,23,42,0.9); color:var(--text-main); font-size:0.8rem; outline:none; cursor:pointer;">
                <!-- Dynamically loaded -->
            </select>
        </div>
        <div class="form-group" style="margin-bottom:12px; display:flex; gap:12px; flex-wrap:wrap;">
            <label style="display:flex; align-items:center; gap:6px; font-size:0.8rem; color:var(--text-muted); cursor:pointer;">
                <input type="checkbox" id="compose-opt-laravel-db-mysql" checked style="cursor:pointer; width:14px; height:14px; margin:0;"> Add MySQL Database
            </label>
            <label style="display:flex; align-items:center; gap:6px; font-size:0.8rem; color:var(--text-muted); cursor:pointer;">
                <input type="checkbox" id="compose-opt-laravel-db-postgres" style="cursor:pointer; width:14px; height:14px; margin:0;"> Add PostgreSQL
            </label>
        </div>
      `;
      html += getTemplateTipHtml("laravel");
      optionsDiv.innerHTML = html;

      // Fetch dynamic PHP tags
      const serverSelect = document.getElementById("compose-opt-laravel-server");
      const loadPhpTags = () => {
        // FrankenPHP tags look like: latest-php8.3-alpine or latest-php8.4-alpine
        loadDynamicDockerTags("dunglas/frankenphp", "compose-opt-laravel-phpver", ["latest-php8.3-alpine", "latest-php8.2-alpine", "latest-php8.4-alpine"], /^(?:1|latest)-php(?:8\.[1234])-alpine$/);
      };

      serverSelect.addEventListener("change", loadPhpTags);
      loadPhpTags();

    } else if (template === "dotnet") {
      html = `
        <div class="form-group" style="margin-bottom:12px;">
            <label style="display:block; margin-bottom:4px; font-weight:500; font-size:0.8rem; color:var(--text-muted);">.NET Version</label>
            <select id="compose-opt-dotnet-ver" style="width:100%; padding:8px; border-radius:4px; border:1px solid var(--card-border); background:rgba(15,23,42,0.9); color:var(--text-main); font-size:0.8rem; outline:none; cursor:pointer;">
                <!-- Dynamically loaded -->
            </select>
        </div>
        <div class="form-group" style="margin-bottom:12px; display:flex; gap:12px; flex-wrap:wrap;">
            <label style="display:flex; align-items:center; gap:6px; font-size:0.8rem; color:var(--text-muted); cursor:pointer;">
                <input type="checkbox" id="compose-opt-dotnet-db-mssql" style="cursor:pointer; width:14px; height:14px; margin:0;"> Add SQL Server
            </label>
            <label style="display:flex; align-items:center; gap:6px; font-size:0.8rem; color:var(--text-muted); cursor:pointer;">
                <input type="checkbox" id="compose-opt-dotnet-db-postgres" style="cursor:pointer; width:14px; height:14px; margin:0;"> Add PostgreSQL
            </label>
        </div>
      `;
      html += getTemplateTipHtml("dotnet");
      optionsDiv.innerHTML = html;
      loadDynamicDockerTags("dotnet/aspnet", "compose-opt-dotnet-ver", ["10.0-alpine", "8.0-alpine", "9.0-alpine"], /^(?:10\.0|8\.0|9\.0)-alpine$/);

    } else if (template === "node") {
      html = `
        <div class="form-group" style="margin-bottom:12px;">
            <label style="display:block; margin-bottom:4px; font-weight:500; font-size:0.8rem; color:var(--text-muted);">Node Version</label>
            <select id="compose-opt-node-ver" style="width:100%; padding:8px; border-radius:4px; border:1px solid var(--card-border); background:rgba(15,23,42,0.9); color:var(--text-main); font-size:0.8rem; outline:none; cursor:pointer;">
                <!-- Dynamically loaded -->
            </select>
        </div>
        <div class="form-group" style="margin-bottom:12px; display:flex; gap:12px; flex-wrap:wrap;">
            <label style="display:flex; align-items:center; gap:6px; font-size:0.8rem; color:var(--text-muted); cursor:pointer;">
                <input type="checkbox" id="compose-opt-node-db-mongo" style="cursor:pointer; width:14px; height:14px; margin:0;"> Add MongoDB
            </label>
            <label style="display:flex; align-items:center; gap:6px; font-size:0.8rem; color:var(--text-muted); cursor:pointer;">
                <input type="checkbox" id="compose-opt-node-db-postgres" style="cursor:pointer; width:14px; height:14px; margin:0;"> Add PostgreSQL
            </label>
        </div>
      `;
      html += getTemplateTipHtml("node");
      optionsDiv.innerHTML = html;
      loadDynamicDockerTags("node", "compose-opt-node-ver", ["20-alpine", "22-alpine", "18-alpine"], /^(?:20|22|18)-alpine$/);

    } else {
      html = `
        <div class="form-group" style="margin-bottom:12px; display:flex; gap:12px; flex-wrap:wrap;">
            <label style="display:flex; align-items:center; gap:6px; font-size:0.8rem; color:var(--text-muted); cursor:pointer;">
                <input type="checkbox" id="compose-opt-generic-db-mysql" style="cursor:pointer; width:14px; height:14px; margin:0;"> Add MySQL
            </label>
            <label style="display:flex; align-items:center; gap:6px; font-size:0.8rem; color:var(--text-muted); cursor:pointer;">
                <input type="checkbox" id="compose-opt-generic-db-postgres" style="cursor:pointer; width:14px; height:14px; margin:0;"> Add PostgreSQL
            </label>
        </div>
      `;
      html += getTemplateTipHtml(template);
      optionsDiv.innerHTML = html;
    }
  });
}

export function openCreateComposeModal() {
  const modal = document.getElementById("create-compose-modal");
  if (modal) {
    document.getElementById("new-compose-project-name").value = "";
    const templateSelect = document.getElementById("new-compose-project-template");
    templateSelect.value = "custom";
    const optionsDiv = document.getElementById("compose-template-options");
    if (optionsDiv) {
      optionsDiv.innerHTML = "";
      optionsDiv.style.display = "none";
    }
    
    if (!templateSelect.dataset.listenerAdded) {
      initComposeTemplateOptionsListener();
      templateSelect.dataset.listenerAdded = "true";
    }
    
    modal.classList.add("active");
  }
}

export function closeCreateComposeModal() {
  const modal = document.getElementById("create-compose-modal");
  if (modal) {
    modal.classList.remove("active");
  }
}

export function createComposeProject() {
  openCreateComposeModal();
}

export function submitCreateComposeProject() {
  const nameInput = document.getElementById("new-compose-project-name");
  const templateSelect = document.getElementById("new-compose-project-template");
  
  if (!nameInput || !templateSelect) return;
  
  const name = nameInput.value.trim();
  const template = templateSelect.value;
  
  if (!name) {
    showToast("error", "Project name is required");
    return;
  }
  
  const cleanName = name.toLowerCase().replace(/[^a-z0-9-_]/g, "");
  if (!cleanName) {
    showToast("error", "Invalid project name");
    return;
  }

  let defaultYaml = "";
  if (template === "node") {
    const nodeVer = document.getElementById("compose-opt-node-ver")?.value || "20-alpine";
    const addMongo = document.getElementById("compose-opt-node-db-mongo")?.checked;
    const addPostgres = document.getElementById("compose-opt-node-db-postgres")?.checked;

    defaultYaml = `version: '3.8'\n\nservices:\n  node-app:\n    image: node:${nodeVer}\n    container_name: ${cleanName}_node_app\n    command: node /app/server.js\n    ports:\n      - "3000:3000"\n    volumes:\n      - /var/lib/zeno-container/volumes/${cleanName}_app:/app\n    environment:\n      NODE_ENV: production\n      PORT: 3000\n    memory: 512m\n    cpus: 0.5\n`;

    if (addMongo) {
      defaultYaml += `\n  mongodb:\n    image: mongo:latest\n    container_name: ${cleanName}_mongodb\n    ports:\n      - "27017:27017"\n    volumes:\n      - /var/lib/zeno-container/volumes/${cleanName}_mongodb:/data/db\n    restart: always\n`;
    }
    if (addPostgres) {
      defaultYaml += `\n  postgres:\n    image: postgres:latest\n    container_name: ${cleanName}_postgres\n    ports:\n      - "5432:5432"\n    environment:\n      POSTGRES_USER: root\n      POSTGRES_PASSWORD: secretpassword\n      POSTGRES_DB: ${cleanName}_db\n    volumes:\n      - /var/lib/zeno-container/volumes/${cleanName}_postgres:/var/lib/postgresql/data\n    restart: always\n`;
    }

  } else if (template === "laravel") {
    const phpServer = document.getElementById("compose-opt-laravel-server")?.value || "frankenphp";
    const phpVer = document.getElementById("compose-opt-laravel-phpver")?.value || "8.3";
    const addMysql = document.getElementById("compose-opt-laravel-db-mysql")?.checked;
    const addPostgres = document.getElementById("compose-opt-laravel-db-postgres")?.checked;

    let appImage = "";
    if (phpServer === "frankenphp") {
      appImage = `dunglas/frankenphp:${phpVer}`;
    } else if (phpServer === "fpm-nginx") {
      appImage = `webdevops/php-nginx:${phpVer}`;
    } else {
      appImage = `webdevops/php-apache:${phpVer}`;
    }

    defaultYaml = `version: '3.8'\n\nservices:\n  php-app:\n    image: ${appImage}\n    container_name: ${cleanName}_php_app\n    ports:\n      - "8000:80"\n    volumes:\n      - /var/lib/zeno-container/volumes/${cleanName}_app:/app\n    environment:\n      SERVER_NAME: :80\n      APP_ENV: production\n      APP_DEBUG: 'false'\n    memory: 512m\n    cpus: 0.5\n`;

    if (addMysql) {
      defaultYaml += `\n  mysql:\n    image: mysql:8.0\n    container_name: ${cleanName}_mysql\n    ports:\n      - "3306:3306"\n    environment:\n      MYSQL_ROOT_PASSWORD: secretpassword\n      MYSQL_DATABASE: ${cleanName}_db\n    volumes:\n      - /var/lib/zeno-container/volumes/${cleanName}_mysql:/var/lib/mysql\n    restart: always\n`;
    }
    if (addPostgres) {
      defaultYaml += `\n  postgres:\n    image: postgres:latest\n    container_name: ${cleanName}_postgres\n    ports:\n      - "5432:5432"\n    environment:\n      POSTGRES_USER: root\n      POSTGRES_PASSWORD: secretpassword\n      POSTGRES_DB: ${cleanName}_db\n    volumes:\n      - /var/lib/zeno-container/volumes/${cleanName}_postgres:/var/lib/postgresql/data\n    restart: always\n`;
    }

  } else if (template === "dotnet") {
    const dotnetVer = document.getElementById("compose-opt-dotnet-ver")?.value || "10.0-alpine";
    const addMssql = document.getElementById("compose-opt-dotnet-db-mssql")?.checked;
    const addPostgres = document.getElementById("compose-opt-dotnet-db-postgres")?.checked;

    defaultYaml = `version: '3.8'\n\nservices:\n  dotnet-app:\n    image: mcr.microsoft.com/dotnet/aspnet:${dotnetVer}\n    container_name: ${cleanName}_dotnet_app\n    command: dotnet /app/app-name.dll\n    ports:\n      - "5000:8080"\n    volumes:\n      - /var/lib/zeno-container/volumes/${cleanName}_app:/app\n    environment:\n      ASPNETCORE_ENVIRONMENT: Production\n      ASPNETCORE_URLS: http://+:8080\n    memory: 512m\n    cpus: 0.5\n`;

    if (addMssql) {
      defaultYaml += `\n  mssql:\n    image: mcr.microsoft.com/mssql/server:2022-latest\n    container_name: ${cleanName}_mssql\n    ports:\n      - "1433:1433"\n    environment:\n      ACCEPT_EULA: "Y"\n      MSSQL_SA_PASSWORD: "SecretPassword123!"\n    restart: always\n`;
    }
    if (addPostgres) {
      defaultYaml += `\n  postgres:\n    image: postgres:latest\n    container_name: ${cleanName}_postgres\n    ports:\n      - "5432:5432"\n    environment:\n      POSTGRES_USER: root\n      POSTGRES_PASSWORD: secretpassword\n      POSTGRES_DB: ${cleanName}_db\n    volumes:\n      - /var/lib/zeno-container/volumes/${cleanName}_postgres:/var/lib/postgresql/data\n    restart: always\n`;
    }

  } else {
    // Other runtimes
    const addMysql = document.getElementById("compose-opt-generic-db-mysql")?.checked;
    const addPostgres = document.getElementById("compose-opt-generic-db-postgres")?.checked;

    if (template === "python") {
      defaultYaml = `version: '3.8'\n\nservices:\n  python-app:\n    image: python:3.11-alpine\n    container_name: ${cleanName}_python_app\n    command: uvicorn main:app --host 0.0.0.0 --port 8000\n    ports:\n      - "8000:80"\n    volumes:\n      - /var/lib/zeno-container/volumes/${cleanName}_app:/app\n    environment:\n      PYTHONUNBUFFERED: 1\n      ENV: production\n    memory: 512m\n    cpus: 0.5\n`;
    } else if (template === "go") {
      defaultYaml = `version: '3.8'\n\nservices:\n  go-app:\n    image: alpine:latest\n    container_name: ${cleanName}_go_app\n    command: /app/main-binary\n    ports:\n      - "8080:8080"\n    volumes:\n      - /var/lib/zeno-container/volumes/${cleanName}_app:/app\n    environment:\n      APP_ENV: production\n    memory: 256m\n    cpus: 0.5\n`;
    } else if (template === "ruby") {
      defaultYaml = `version: '3.8'\n\nservices:\n  rails-app:\n    image: ruby:3.2-alpine\n    container_name: ${cleanName}_rails_app\n    command: rails server -b 0.0.0.0\n    ports:\n      - "3000:3000"\n    volumes:\n      - /var/lib/zeno-container/volumes/${cleanName}_app:/app\n    environment:\n      RAILS_ENV: development\n    memory: 512m\n    cpus: 0.5\n`;
    } else if (template === "java") {
      defaultYaml = `version: '3.8'\n\nservices:\n  spring-app:\n    image: eclipse-temurin:17-jre-alpine\n    container_name: ${cleanName}_spring_app\n    command: java -jar /app/app.jar\n    ports:\n      - "8080:8080"\n    volumes:\n      - /var/lib/zeno-container/volumes/${cleanName}_app:/app\n    memory: 1g\n    cpus: 1.0\n`;
    } else if (template === "rust") {
      defaultYaml = `version: '3.8'\n\nservices:\n  rust-app:\n    image: rust:alpine\n    container_name: ${cleanName}_rust_app\n    command: /app/release-binary\n    ports:\n      - "8080:8080"\n    volumes:\n      - /var/lib/zeno-container/volumes/${cleanName}_app:/app\n    memory: 512m\n    cpus: 0.5\n`;
    } else {
      defaultYaml = `version: '3.8'\n\nservices:\n  app:\n    image: alpine:latest\n    container_name: ${cleanName}_custom_app\n    command: tail -f /dev/null\n    restart: always\n`;
    }

    if (addMysql) {
      defaultYaml += `\n  mysql:\n    image: mysql:8.0\n    container_name: ${cleanName}_mysql\n    ports:\n      - "3306:3306"\n    environment:\n      MYSQL_ROOT_PASSWORD: secretpassword\n      MYSQL_DATABASE: ${cleanName}_db\n    volumes:\n      - /var/lib/zeno-container/volumes/${cleanName}_mysql:/var/lib/mysql\n    restart: always\n`;
    }
    if (addPostgres) {
      defaultYaml += `\n  postgres:\n    image: postgres:latest\n    container_name: ${cleanName}_postgres\n    ports:\n      - "5432:5432"\n    environment:\n      POSTGRES_USER: root\n      POSTGRES_PASSWORD: secretpassword\n      POSTGRES_DB: ${cleanName}_db\n    volumes:\n      - /var/lib/zeno-container/volumes/${cleanName}_postgres:/var/lib/postgresql/data\n    restart: always\n`;
    }
  }

  closeCreateComposeModal();
  const csrf = getCSRFToken();

  fetch("/api/containers/compose", {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      "X-CSRF-Token": csrf,
    },
    body: JSON.stringify({
      action: "save",
      yaml: defaultYaml,
      project_name: cleanName,
      file: "docker-compose.yml"
    }),
  })
    .then((res) => res.json())
    .then((res) => {
      if (res.success) {
        showToast("success", `Project '${cleanName}' created successfully`);
        activeProject = cleanName;
        // Also create empty .env file by default
        return fetch("/api/containers/compose", {
          method: "POST",
          headers: {
            "Content-Type": "application/json",
            "X-CSRF-Token": csrf,
          },
          body: JSON.stringify({
            action: "save",
            yaml: "# Environment variables go here\n",
            project_name: cleanName,
            file: ".env"
          })
        });
      } else {
        throw new Error(res.error || "Failed to create project");
      }
    })
    .then((res) => res.json())
    .then(() => {
      loadComposeProjects();
      loadComposeYaml(cleanName, activeFile);
    })
    .catch((err) => {
      console.error("Error creating project:", err);
      showToast("error", err.message || "Failed to create project");
    });
}

export function saveComposeYaml() {
  const yamlTextarea = document.getElementById("compose-yaml-input");
  if (!yamlTextarea) return;

  const content = yamlTextarea.value;
  const csrf = getCSRFToken();

  // Validate .env format if saving .env file
  if (activeFile === ".env") {
    const lines = content.split('\n');
    for (let i = 0; i < lines.length; i++) {
      const line = lines[i].trim();
      if (!line || line.startsWith('#')) continue;

      if (!/^[a-zA-Z_][a-zA-Z0-9_]*=.*$/.test(line)) {
        if (!confirm(`Warning: Line ${i + 1} ("${line}") does not follow the standard KEY=VALUE format. Save anyway?`)) {
          return;
        }
      }
    }
  }

  fetch("/api/containers/compose", {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      "X-CSRF-Token": csrf,
    },
    body: JSON.stringify({
      action: "save",
      yaml: content,
      project_name: activeProject,
      file: activeFile
    }),
  })
    .then((res) => res.json())
    .then((res) => {
      if (res.success) {
        showToast("success", `'${activeFile}' saved successfully`);
      } else {
        showToast("error", res.error || `Failed to save ${activeFile}`);
      }
    })
    .catch((err) => {
      console.error("Error saving file:", err);
      showToast("error", "Failed to save file");
    });
}

// ─── Env Editor Tab Switcher ─────────────────────────────────────────

export function switchComposeFileTab(file) {
  activeFile = file;

  const tabYaml = document.getElementById("compose-file-tab-yaml");
  const tabEnv = document.getElementById("compose-file-tab-env");

  if (tabYaml && tabEnv) {
    if (file === "docker-compose.yml") {
      tabYaml.style.background = "rgba(59,130,246,0.1)";
      tabYaml.style.color = "var(--accent-primary)";
      tabYaml.style.borderColor = "rgba(59,130,246,0.25)";

      tabEnv.style.background = "transparent";
      tabEnv.style.color = "var(--text-muted)";
      tabEnv.style.borderColor = "transparent";
    } else {
      tabEnv.style.background = "rgba(59,130,246,0.1)";
      tabEnv.style.color = "var(--accent-primary)";
      tabEnv.style.borderColor = "rgba(59,130,246,0.25)";

      tabYaml.style.background = "transparent";
      tabYaml.style.color = "var(--text-muted)";
      tabYaml.style.borderColor = "transparent";
    }
  }

  loadComposeYaml(activeProject, file);
}

// ─── Delete Project ──────────────────────────────────────────────────

export function deleteComposeProject() {
  if (activeProject === "default") {
    showToast("error", "Cannot delete default project");
    return;
  }

  if (!confirm(`Are you sure you want to delete the project '${activeProject}'? All containers will be stopped and files removed.`)) {
    return;
  }

  const csrf = getCSRFToken();

  // First stop containers
  const resultDiv = document.getElementById("compose-result");
  if (resultDiv) {
    resultDiv.textContent = `Stopping compose project ${activeProject} and deleting files...\n`;
  }

  fetch("/api/containers/compose", {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      "X-CSRF-Token": csrf,
    },
    body: JSON.stringify({ action: "down", yaml: "", project_name: activeProject }),
  })
    .then((res) => res.json())
    .then(() => {
      // Delete project folder
      return fetch("/api/containers/compose/delete", {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
          "X-CSRF-Token": csrf,
        },
        body: JSON.stringify({ project_name: activeProject }),
      });
    })
    .then((res) => res.json())
    .then((res) => {
      if (res.success) {
        showToast("success", `Project '${activeProject}' deleted successfully`);
        activeProject = "default";
        activeFile = "docker-compose.yml";
        switchComposeFileTab("docker-compose.yml");
        loadComposeProjects();
        loadComposeYaml("default", "docker-compose.yml");
      } else {
        showToast("error", res.message || "Failed to delete project");
      }
    })
    .catch((err) => {
      console.error("Error deleting project:", err);
      showToast("error", "Failed to delete project");
    });
}

// ─── Proxy & Log Console Utilities ───────────────────────────────────

export function exposeComposeViaProxy() {
  const yamlText = document.getElementById("compose-yaml-input").value;
  
  // Regex to match port pattern - "8080:80" or - 3000:3000
  const portRegex = /-\s*["']?(\d+):(\d+)["']?/g;
  let ports = [];
  let match;
  while ((match = portRegex.exec(yamlText)) !== null) {
    ports.push(parseInt(match[1], 10));
  }

  if (ports.length === 0) {
    showToast("error", "No exposed ports found in YAML configuration");
    return;
  }

  const targetPort = ports[0];
  const nameRegex = /container_name:\s*([a-zA-Z0-9_-]+)/;
  const nameMatch = nameRegex.exec(yamlText);
  const suggestedName = nameMatch ? nameMatch[1] : activeProject;

  // Open proxy modal (which resets inputs)
  if (typeof window.openAddProxyModal === "function") {
    window.openAddProxyModal();
  }

  // Pre-fill values
  const nameInput = document.getElementById('proxy-name');
  const domInput = document.getElementById('proxy-domain');
  const targetInput = document.getElementById('proxy-target');

  if (nameInput) nameInput.value = suggestedName;
  if (domInput) domInput.value = suggestedName + '.local';
  if (targetInput) targetInput.value = `http://127.0.0.1:${targetPort}`;
  
  // Show navigation helper toast
  showToast("info", "Reverse proxy configured with exposed port " + targetPort + ". Switch to the Proxy tab to finalize.");
}

export function clearComposeConsole() {
  const resultDiv = document.getElementById("compose-result");
  if (resultDiv) {
    resultDiv.textContent = "";
  }
}
