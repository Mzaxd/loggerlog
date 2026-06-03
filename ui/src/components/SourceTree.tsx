import type { IndexStats } from "../lib/api";

interface Props {
  stats: IndexStats | null;
}

export function SourceTree({ stats }: Props) {
  return (
    <div className="p-3">
      <h3 className="text-xs font-medium text-gray-400 uppercase tracking-wider mb-3">Log Sources</h3>

      {!stats || stats.files.length === 0 ? (
        <div className="text-gray-500 text-xs text-center py-8">
          <p>No sources indexed.</p>
          <p className="mt-1">Use the CLI to add directories:</p>
          <code className="mt-2 block bg-gray-800 px-2 py-1 rounded text-gray-300">
            loggerlog config add-dir /path/to/logs
          </code>
        </div>
      ) : (
        <div className="space-y-1">
          {stats.files.map((file, i) => (
            <div key={i} className="flex items-start gap-2 p-2 rounded hover:bg-gray-800/50 text-xs cursor-pointer group">
              <div className="w-2 h-2 mt-1 rounded-full shrink-0" style={{
                backgroundColor: file.format === "json" ? "#60a5fa" : file.format === "log4j" || file.format === "logback" ? "#34d399" : "#a78bfa",
              }} />
              <div className="min-w-0 flex-1">
                <div className="text-gray-200 truncate group-hover:text-gray-100" title={file.path}>
                  {file.path.split("/").pop()}
                </div>
                <div className="text-gray-500 mt-0.5">
                  {file.entries.toLocaleString()} entries · {file.size_mb} · {file.format}
                </div>
              </div>
            </div>
          ))}
        </div>
      )}

      {stats && (
        <div className="mt-6 pt-3 border-t border-gray-800">
          <h3 className="text-xs font-medium text-gray-400 uppercase tracking-wider mb-2">Index Status</h3>
          <div className="space-y-1.5 text-xs text-gray-400">
            <div className="flex justify-between">
              <span>Files</span>
              <span className="text-gray-300">{stats.total_files}</span>
            </div>
            <div className="flex justify-between">
              <span>Entries</span>
              <span className="text-gray-300">{stats.total_entries.toLocaleString()}</span>
            </div>
            <div className="flex justify-between">
              <span>Database</span>
              <span className="text-gray-300">{stats.database_size_mb}</span>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
