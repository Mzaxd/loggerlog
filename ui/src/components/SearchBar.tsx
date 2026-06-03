import { useState, useCallback, useRef, useEffect } from "react";
import { Search, RefreshCw, FolderPlus } from "lucide-react";
import { searchLogs, getIndexStats, updateIndex, addDirectory } from "../lib/api";
import type { SearchResultSet, IndexStats } from "../lib/api";

interface Props {
  onResults: (data: SearchResultSet) => void;
  onStatsLoaded: (stats: IndexStats) => void;
}

export function SearchBar({ onResults, onStatsLoaded }: Props) {
  const [query, setQuery] = useState("");
  const [level, setLevel] = useState("");
  const [searching, setSearching] = useState(false);
  const debounceRef = useRef<ReturnType<typeof setTimeout>>(undefined);

  const doSearch = useCallback(async (q: string) => {
    if (!q.trim()) return;
    setSearching(true);
    try {
      const data = await searchLogs(q, {
        levels: level ? [level] : [],
        limit: 200,
      });
      onResults(data);
    } catch (e) {
      console.error("Search failed:", e);
    } finally {
      setSearching(false);
    }
  }, [level, onResults]);

  // Debounced search
  useEffect(() => {
    if (debounceRef.current) clearTimeout(debounceRef.current);
    debounceRef.current = setTimeout(() => doSearch(query), 300);
    return () => { if (debounceRef.current) clearTimeout(debounceRef.current); };
  }, [query, doSearch]);

  const handleRefreshStats = async () => {
    try {
      await updateIndex();
      const stats = await getIndexStats();
      onStatsLoaded(stats);
    } catch (e) {
      console.error("Refresh failed:", e);
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter") {
      doSearch(query);
    }
  };

  return (
    <div className="flex items-center gap-3">
      <div className="flex-1 flex items-center gap-2 bg-gray-800 rounded-lg px-3 py-2">
        <Search className="w-4 h-4 text-gray-400" />
        <input
          type="text"
          placeholder="Search logs... (FTS query, or level=ERROR, regex:pattern)"
          className="flex-1 bg-transparent text-gray-100 placeholder-gray-500 outline-none text-sm"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          onKeyDown={handleKeyDown}
        />
        <select
          className="bg-gray-700 text-gray-300 text-xs rounded px-2 py-1 outline-none"
          value={level}
          onChange={(e) => setLevel(e.target.value)}
        >
          <option value="">All Levels</option>
          <option value="ERROR">ERROR</option>
          <option value="WARN">WARN</option>
          <option value="INFO">INFO</option>
          <option value="DEBUG">DEBUG</option>
          <option value="TRACE">TRACE</option>
        </select>
      </div>
      <button
        onClick={handleRefreshStats}
        className="p-2 text-gray-400 hover:text-gray-200 hover:bg-gray-800 rounded-lg transition-colors"
        title="Refresh Index"
      >
        <RefreshCw className="w-4 h-4" />
      </button>
    </div>
  );
}
