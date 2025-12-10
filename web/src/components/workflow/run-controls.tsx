'use client';

import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';

interface RunControlsProps {
  isRunning: boolean;
  runId?: string;
  onRun: () => void;
  onStop: () => void;
}

export default function RunControls({ isRunning, runId, onRun, onStop }: RunControlsProps) {
  return (
    <div className="flex items-center justify-between p-4 border-b bg-background">
      <div className="flex items-center gap-4">
        <h1 className="text-xl font-bold">Workflow Runner</h1>
        {runId && (
          <Badge variant="outline" className="font-mono text-xs">
            {runId}
          </Badge>
        )}
      </div>
      <div className="flex items-center gap-2">
        {isRunning ? (
          <Button variant="destructive" onClick={onStop}>
            Stop
          </Button>
        ) : (
          <Button onClick={onRun}>
            Run Workflows
          </Button>
        )}
      </div>
    </div>
  );
}
