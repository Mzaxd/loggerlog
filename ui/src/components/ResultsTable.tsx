import type { SearchResult } from "../lib/api";

interface Props {
  results: SearchResult[];
  totalCount: number;
  elapsedMs: number;
  onSelect: (entry: SearchResult | null) => void;
}

const levelColors: Record<string, string> = {
  ERROR: "text-red-400 bg-red-400/10",
  FATAL: "text-red-400 bg-red-400/10",
  SEVERE: "text-red-400 bg-red-400/10",
  WARN: "text-yellow-400 bg-yellow-400/10",
  WARNING: "text-yellow-400 bg-yellow-400/10",
  INFO: "text-green-400 bg-green-400/10",
  DEBUG: "text-blue-400 bg-blue-400/10",
  TRACE: "text-gray-400 bg-gray-400/10",
};

export function ResultsTable({ results, totalCount, elapsedMs, onSelect }: Props) {
  if (results.length === 0) {
    return (
      <div className="flex-1 flex items-center justify-center text-gray-500">
        <div className="text-center">
          <p className="text-lg mb-1">No results</p>
          <p className="text-sm">Enter a search query above to get started</p>
        </div>
      </div>
    );
  }

  return (
    <div className="flex-1 flex flex-col overflow-hidden">
      <div className="px-3 py-1.5 text-xs text-gray-400 border-b border-gray-800 bg-gray-900/50 flex justify-between">
        <span>Showing {results.length} of {totalCount.toLocaleString()} results</span>
        <span>{elapsedMs}ms</span>
      </div>
      <div className="flex-1 overflow-y-auto">
        <table className="w-full text-xs">
          <thead className="sticky top-0 bg-gray-900 text-gray-400 uppercase">
            <tr className="border-b border-gray-800">
              <th className="text-left px-3 py-1.5 w-44">Timestamp</th>
              <th className="text-left px-2 py-1.5 w-16">Level</th>
              <th className="text-left px-2 py-1.5 w-40">Source</th>
              <th className="text-left px-2 py-1.5 w-12">Line</th>
              <th className="text-left px-3 py-1.5">Message</th>
            </tr>
          </thead>
          <tbody>
            {results.map((entry) => (
              <tr
                key={entry.id}
                className="border-b border-gray-800/50 hover:bg-gray-800/50 cursor-pointer"
                onClick={() => onSelect(entry)}
              >
                <td className="px-3 py-1.5 text-gray-300 font-mono whitespace-nowrap">
                  {formatTimestamp(entry.timestamp)}
                </td>
                <td className="px-2 py-1.5">
                  <span className={`px-1.5 py-0.5 rounded text-[10px] font-medium ${levelColors[entry.level || ""] || "text-gray-400 bg-gray-400/10"}`}>
                    {entry.level || "-"}
                  </span>
                </td>
                <td className="px-2 py-1.5 text-gray-400 truncate" title={entry.source}>
                  {shortenPath(entry.source)}
                </td>
                <td className="px-2 py-1.5 text-gray-500">{entry.line_number}</td>
                <td className="px-3 py-1.5 text-gray-200 truncate">{entry.message}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}

function formatTimestamp(ts: string | null): string {
  if (!ts) return "-";
  try {
    const d = new Date(ts);
    return d.toLocaleString("en-US", {
      month: "2-digit", day: "2-digit", hour: "2-digit", minute: "2-digit", second: "2-digit",
      fractionalSecondDigits: 3,
    });
  } catch {
    return ts;
  }
}

function shortenPath(path: string): string {
  const parts = path.split("/");
  if (parts.length > 3) return parts.slice(-3).join("/");
  return path;
}
