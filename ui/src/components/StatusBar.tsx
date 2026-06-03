import type { IndexStats } from "../lib/api";

interface Props {
  stats: IndexStats | null;
  resultCount: number;
  elapsedMs: number;
}

export function StatusBar({ stats, resultCount, elapsedMs }: Props) {
  return (
    <div className="h-7 border-t border-gray-800 px-4 flex items-center gap-4 text-[10px] text-gray-500 bg-gray-900 shrink-0">
      <span>LoggerLog v0.1.0</span>
      {stats && (
        <>
          <span className="w-px h-3 bg-gray-700" />
          <span>Indexed {stats.total_files} files</span>
          <span>·</span>
          <span>{stats.total_entries.toLocaleString()} entries</span>
          <span>·</span>
          <span>DB {stats.database_size_mb}</span>
        </>
      )}
      {resultCount > 0 && (
        <>
          <span className="w-px h-3 bg-gray-700" />
          <span>{resultCount.toLocaleString()} results</span>
          <span>·</span>
          <span>{elapsedMs}ms</span>
        </>
      )}
    </div>
  );
}
