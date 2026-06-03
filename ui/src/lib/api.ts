import { invoke } from "@tauri-apps/api/core";

export interface SearchResult {
  id: number;
  source: string;
  line_number: number;
  byte_offset: number;
  timestamp: string | null;
  level: string | null;
  thread: string | null;
  logger: string | null;
  message: string;
  fields_json: string | null;
  raw: string;
}

export interface SearchResultSet {
  total_count: number;
  returned_count: number;
  offset: number;
  elapsed_ms: number;
  results: SearchResult[];
}

export interface IndexStats {
  database_size_mb: string;
  total_files: number;
  total_entries: number;
  files: FileStat[];
}

export interface FileStat {
  path: string;
  format: string;
  entries: number;
  size_mb: string;
  byte_offset: number;
}

export interface ConfigInfo {
  general: {
    database_path: string;
    max_file_size: string;
    watch_interval: string;
  };
  sources: {
    directories: DirectorySource[];
  };
}

export interface DirectorySource {
  path: string;
  recursive: boolean;
  encoding: string;
}

export async function searchLogs(query: string, options: {
  levels?: string[];
  source?: string;
  after?: string;
  before?: string;
  thread?: string;
  regex?: boolean;
  limit?: number;
}): Promise<SearchResultSet> {
  return invoke<SearchResultSet>("search_logs", {
    query,
    levels: options.levels || [],
    source: options.source || null,
    after: options.after || null,
    before: options.before || null,
    thread: options.thread || null,
    useRegex: options.regex || false,
    limit: options.limit || 100,
  });
}

export async function getIndexStats(): Promise<IndexStats> {
  return invoke<IndexStats>("get_index_stats");
}

export async function addDirectory(path: string, recursive: boolean, encoding: string): Promise<void> {
  return invoke("add_directory", { path, recursive, encoding });
}

export async function removeDirectory(path: string): Promise<void> {
  return invoke("remove_directory", { path });
}

export async function updateIndex(): Promise<void> {
  return invoke("update_index");
}

export async function getConfig(): Promise<ConfigInfo> {
  return invoke<ConfigInfo>("get_config");
}
