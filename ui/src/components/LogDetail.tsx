import type { SearchResult } from "../lib/api";

interface Props {
  entry: SearchResult | null;
}

export function LogDetail({ entry }: Props) {
  if (!entry) {
    return (
      <div className="h-full flex items-center justify-center text-gray-500 text-sm">
        <p>Click a result to view details</p>
      </div>
    );
  }

  let fields: Record<string, unknown> | null = null;
  if (entry.fields_json) {
    try { fields = JSON.parse(entry.fields_json); } catch {}
  }

  return (
    <div className="p-4 space-y-4 text-sm">
      <h3 className="text-xs font-medium text-gray-400 uppercase tracking-wider">Log Detail</h3>

      <div className="space-y-3">
        <DetailRow label="Source" value={entry.source} />
        <DetailRow label="Line" value={String(entry.line_number)} />
        <DetailRow label="Timestamp" value={entry.timestamp || "-"} />
        <DetailRow label="Level" value={entry.level || "-"} level={entry.level} />
        {entry.thread && <DetailRow label="Thread" value={entry.thread} />}
        {entry.logger && <DetailRow label="Logger" value={entry.logger} />}
      </div>

      <div>
        <h4 className="text-xs font-medium text-gray-400 mb-1">Message</h4>
        <div className="bg-gray-800 rounded-lg p-3 text-gray-200 whitespace-pre-wrap break-all font-mono text-xs">
          {entry.message}
        </div>
      </div>

      <div>
        <h4 className="text-xs font-medium text-gray-400 mb-1">Raw</h4>
        <div className="bg-gray-800 rounded-lg p-3 text-gray-300 whitespace-pre-wrap break-all font-mono text-xs max-h-64 overflow-y-auto">
          {entry.raw}
        </div>
      </div>

      {fields && Object.keys(fields).length > 0 && (
        <div>
          <h4 className="text-xs font-medium text-gray-400 mb-1">Fields</h4>
          <div className="space-y-1">
            {Object.entries(fields).map(([key, val]) => (
              <div key={key} className="flex gap-2 text-xs">
                <span className="text-gray-500 w-24 shrink-0">{key}:</span>
                <span className="text-gray-300 font-mono">{String(val)}</span>
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}

function DetailRow({ label, value, level }: { label: string; value: string; level?: string | null }) {
  const levelColors: Record<string, string> = {
    ERROR: "text-red-400", FATAL: "text-red-400", SEVERE: "text-red-400",
    WARN: "text-yellow-400", WARNING: "text-yellow-400",
    INFO: "text-green-400", DEBUG: "text-blue-400", TRACE: "text-gray-400",
  };

  return (
    <div className="flex gap-2 text-xs">
      <span className="text-gray-500 w-20 shrink-0">{label}:</span>
      <span className={`font-mono ${level ? (levelColors[level] || "text-gray-300") : "text-gray-300"}`}>
        {value}
      </span>
    </div>
  );
}
