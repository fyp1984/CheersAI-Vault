export interface GiteaConfig {
  url: string;
  token: string;
  owner: string;
  repo: string;
  enabled: boolean;
  has_token?: boolean;
}

export interface GiteaStatusResponse {
  enabled: boolean;
  configured: boolean;
  repo_exists: boolean | null;
  config: GiteaConfig;
}

export interface UploadResult {
  success: boolean;
  urls: string[];
  message: string;
}
