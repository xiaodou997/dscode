import { invoke } from "@tauri-apps/api/core";

export type DashboardState = {
  officialApp: {
    installed: boolean;
    running: boolean;
    path: string | null;
    version: string | null;
  };
  officialAuthPresent: boolean;
  codexHome: string;
  endpoint: string;
  provider: {
    activeProvider: string | null;
    doustackConfigured: boolean;
    doustackActive: boolean;
  };
  credentialSaved: boolean;
  credentialError: string | null;
  localRuntime: {
    available: boolean;
    version: string | null;
    tested: boolean;
    testedVersion: string;
  };
  latestBackup: string | null;
};

export type ConfigMutationResult = {
  state: DashboardState;
  changed: boolean;
  configPath: string;
  backupPath: string | null;
};

const browserPreviewState: DashboardState = {
  officialApp: {
    installed: true,
    running: false,
    path: "/Applications/ChatGPT.app",
    version: "26.721.41059",
  },
  officialAuthPresent: true,
  codexHome: "~/.codex",
  endpoint: "https://miao.313619.xyz",
  provider: {
    activeProvider: "doustack",
    doustackConfigured: true,
    doustackActive: true,
  },
  credentialSaved: true,
  credentialError: null,
  localRuntime: {
    available: true,
    version: "0.146.0",
    tested: true,
    testedVersion: "0.146.0",
  },
  latestBackup: "~/.dscode/backups/official-codex/001785374000000000000000000000-997",
};

function isTauri() {
  return "__TAURI_INTERNALS__" in window;
}

export async function loadDashboard(): Promise<DashboardState> {
  if (!isTauri()) return browserPreviewState;
  return invoke<DashboardState>("dashboard_state");
}

export async function configureDouStack(apiKey: string): Promise<ConfigMutationResult> {
  if (!isTauri()) {
    return {
      state: {
        ...browserPreviewState,
        credentialSaved: true,
        provider: {
          activeProvider: "doustack",
          doustackConfigured: true,
          doustackActive: true,
        },
      },
      changed: true,
      configPath: "~/.codex/config.toml",
      backupPath: "~/.dscode/backups/official-codex/preview",
    };
  }
  return invoke<ConfigMutationResult>("configure_doustack", { apiKey });
}

export async function restoreLatestConfig(): Promise<ConfigMutationResult> {
  if (!isTauri()) {
    return {
      state: {
        ...browserPreviewState,
        provider: {
          activeProvider: null,
          doustackConfigured: false,
          doustackActive: false,
        },
      },
      changed: true,
      configPath: "~/.codex/config.toml",
      backupPath: browserPreviewState.latestBackup,
    };
  }
  return invoke<ConfigMutationResult>("restore_latest_config");
}

export async function forgetDouStackKey(): Promise<DashboardState> {
  if (!isTauri()) return { ...browserPreviewState, credentialSaved: false };
  return invoke<DashboardState>("forget_doustack_key");
}

export async function previewConfig(): Promise<string> {
  if (!isTauri()) {
    return `model_provider = "doustack"

[model_providers.doustack]
name = "OpenAI"
base_url = "https://miao.313619.xyz"
env_key = "DOUSTACK_API_KEY"
wire_api = "responses"
requires_openai_auth = false
supports_websockets = false
`;
  }
  return invoke<string>("preview_doustack_config");
}

export async function launchOfficialCodex(): Promise<void> {
  if (!isTauri()) return;
  return invoke<void>("launch_official_codex");
}

export async function openOfficialDownload(): Promise<void> {
  if (!isTauri()) {
    window.open("https://openai.com/codex/get-started/", "_blank", "noopener,noreferrer");
    return;
  }
  return invoke<void>("open_official_download");
}
