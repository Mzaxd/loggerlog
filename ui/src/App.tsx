import { useState, useCallback } from "react";
import { SearchBar } from "./components/SearchBar";
import { ResultsTable } from "./components/ResultsTable";
import { LogDetail } from "./components/LogDetail";
import { SourceTree } from "./components/SourceTree";
import { StatusBar } from "./components/StatusBar";
import type { SearchResult, IndexStats } from "./lib/api";

export default function App() {
  const [results, setResults] = useState<SearchResult[]>([]);
  const [totalCount, setTotalCount] = useState(0);
  const [elapsedMs, setElapsedMs] = useState(0);
  const [selectedResult, setSelectedResult] = useState<SearchResult | null>(null);
  const [stats, setStats] = useState<IndexStats | null>(null);

  return (
    <div className="h-screen flex flex-col overflow-hidden bg-gray-950">
      {/* Search bar */}
      <div className="border-b border-gray-800 px-4 py-3">
        <SearchBar
          onResults={(data) => {
            setResults(data.results);
            setTotalCount(data.total_count);
            setElapsedMs(data.elapsed_ms);
          }}
          onStatsLoaded={setStats}
        />
      </div>

      {/* Main content: three panels */}
      <div className="flex-1 flex overflow-hidden">
        {/* Left: Sources */}
        <div className="w-64 border-r border-gray-800 overflow-y-auto bg-gray-900">
          <SourceTree stats={stats} />
        </div>

        {/* Center: Results */}
        <div className="flex-1 flex flex-col overflow-hidden">
          <ResultsTable
            results={results}
            totalCount={totalCount}
            elapsedMs={elapsedMs}
            onSelect={setSelectedResult}
          />
        </div>

        {/* Right: Detail */}
        <div className="w-96 border-l border-gray-800 overflow-y-auto bg-gray-900">
          <LogDetail entry={selectedResult} />
        </div>
      </div>

      {/* Status bar */}
      <StatusBar stats={stats} resultCount={totalCount} elapsedMs={elapsedMs} />
    </div>
  );
}
