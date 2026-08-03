export interface GiteaConfig {
  url: string;
  owner: string;
  repo: string;
  enabled: boolean;
  has_token: boolean;
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
  items: UploadItemResult[];
}

export interface UploadItemResult {
  history_id: string;
  remote_path: string;
  success: boolean;
  url?: string;
  error_code?: string;
}
