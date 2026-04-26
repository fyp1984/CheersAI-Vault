export interface ManagedFile {
  id: string;
  original_name: string;
  masked_name: string;
  file_path: string;
  file_size: number;
  file_type: string;
  masked_count: number;
  gitea_uploaded: boolean;
  gitea_url: string | null;
  created_at: string;
  updated_at: string;
}

export interface FileListResponse {
  files: ManagedFile[];
  total: number;
  page: number;
  page_size: number;
}

export interface FileStatistics {
  total_files: number;
  uploaded_to_gitea: number;
  total_size: number;
}
