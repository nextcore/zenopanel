import { getCSRFToken } from "./utils.js";
import { showToast } from "./toast.js";

// ─── Docker Compose ──────────────────────────────────────────────────

function composeExec(action) {
  const yaml = document.getElementById("compose-yaml-input").value.trim();
  if (!yaml) {
    showToast("error", "Enter compose YAML first");
    return;
  }

  const resultDiv = document.getElementById("compose-result");
  resultDiv.style.display = "block";
  resultDiv.textContent = `Running compose ${action}...\n`;

  const cliArgs = `compose ${action} /tmp/zeno-compose.yml`;
  const csrf = getCSRFToken();

  fetch("/api/containers/compose", {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      "X-CSRF-Token": csrf,
    },
    body: JSON.stringify({ cli_args: cliArgs, yaml }),
  })
    .then((res) => res.json())
    .then((res) => {
      if (res.success) {
        resultDiv.textContent = res.output || "Done.\n";
        showToast("success", `Compose ${action} completed`);
        // Refresh containers list in the background
        import("./containers.js").then((m) => m.loadContainers());
      } else {
        resultDiv.textContent = res.error || "Failed.\n";
        showToast("error", res.message || "Compose failed");
      }
    })
    .catch((err) => {
      resultDiv.textContent = "Network error.\n";
      showToast("error", "Network error");
    });
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
