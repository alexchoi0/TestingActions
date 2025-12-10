'use client';

import { memo } from 'react';
import { Handle, Position, type NodeProps } from '@xyflow/react';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Badge } from '@/components/ui/badge';
import type { WorkflowStatus } from '@/types/workflow';

interface WorkflowNodeData {
  name: string;
  status: WorkflowStatus;
  jobCount: number;
  completedJobs: number;
}

const statusColors: Record<WorkflowStatus, string> = {
  pending: 'bg-zinc-500',
  running: 'bg-blue-500 animate-pulse',
  success: 'bg-green-500',
  failed: 'bg-red-500',
  skipped: 'bg-zinc-400',
};

const statusBorder: Record<WorkflowStatus, string> = {
  pending: 'border-zinc-300',
  running: 'border-blue-500 shadow-lg shadow-blue-500/20',
  success: 'border-green-500',
  failed: 'border-red-500',
  skipped: 'border-zinc-300',
};

function WorkflowNode({ data }: NodeProps) {
  const nodeData = data as unknown as WorkflowNodeData;
  const { name, status, jobCount, completedJobs } = nodeData;

  return (
    <>
      <Handle type="target" position={Position.Left} className="!bg-zinc-400" />
      <Card className={`w-[200px] border-2 ${statusBorder[status]}`}>
        <CardHeader className="p-3 pb-2">
          <div className="flex items-center justify-between gap-2">
            <CardTitle className="text-sm font-medium truncate">{name}</CardTitle>
            <div className={`w-2 h-2 rounded-full ${statusColors[status]}`} />
          </div>
        </CardHeader>
        <CardContent className="p-3 pt-0">
          <div className="flex items-center justify-between text-xs text-muted-foreground">
            <span>Jobs</span>
            <Badge variant="secondary" className="text-xs">
              {completedJobs}/{jobCount}
            </Badge>
          </div>
        </CardContent>
      </Card>
      <Handle type="source" position={Position.Right} className="!bg-zinc-400" />
    </>
  );
}

export default memo(WorkflowNode);
