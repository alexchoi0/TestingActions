'use client';

import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Badge } from '@/components/ui/badge';
import { ScrollArea } from '@/components/ui/scroll-area';
import { Separator } from '@/components/ui/separator';
import type { Workflow, Job, Step, JobStatus, StepStatus } from '@/types/workflow';

const statusVariant: Record<JobStatus | StepStatus | 'paused', 'default' | 'secondary' | 'destructive' | 'outline'> = {
  pending: 'outline',
  running: 'default',
  paused: 'secondary',
  success: 'secondary',
  failed: 'destructive',
  skipped: 'outline',
};

const statusLabel: Record<JobStatus | StepStatus | 'paused', string> = {
  pending: 'Pending',
  running: 'Running',
  paused: 'Paused',
  success: 'Success',
  failed: 'Failed',
  skipped: 'Skipped',
};

interface WorkflowDetailProps {
  workflow: Workflow | null;
}

function StepItem({ step }: { step: Step }) {
  return (
    <div className="flex items-center justify-between py-2 px-3 rounded-md hover:bg-muted/50">
      <div className="flex flex-col gap-1">
        <span className="text-sm font-medium">{step.name}</span>
        <span className="text-xs text-muted-foreground font-mono">{step.uses}</span>
      </div>
      <Badge variant={statusVariant[step.status]} className="text-xs">
        {statusLabel[step.status]}
      </Badge>
    </div>
  );
}

function JobItem({ job }: { job: Job }) {
  return (
    <Card className="mb-3">
      <CardHeader className="p-3 pb-2">
        <div className="flex items-center justify-between">
          <CardTitle className="text-sm">{job.name}</CardTitle>
          <Badge variant={statusVariant[job.status]}>{statusLabel[job.status]}</Badge>
        </div>
      </CardHeader>
      <CardContent className="p-3 pt-0">
        <div className="space-y-1">
          {job.steps.map(step => (
            <StepItem key={step.id} step={step} />
          ))}
        </div>
      </CardContent>
    </Card>
  );
}

export default function WorkflowDetail({ workflow }: WorkflowDetailProps) {
  if (!workflow) {
    return (
      <div className="flex items-center justify-center h-full text-muted-foreground">
        Select a workflow to view details
      </div>
    );
  }

  const jobs = Object.values(workflow.jobs);

  return (
    <div className="h-full flex flex-col">
      <div className="p-4 border-b">
        <div className="flex items-center justify-between">
          <h2 className="text-lg font-semibold">{workflow.name}</h2>
          <Badge variant={statusVariant[workflow.status]} className="text-sm">
            {statusLabel[workflow.status]}
          </Badge>
        </div>
        {workflow.dependsOn.length > 0 && (
          <div className="mt-2 text-xs text-muted-foreground">
            Depends on: {workflow.dependsOn.join(', ')}
          </div>
        )}
      </div>
      <ScrollArea className="flex-1 p-4">
        <div className="space-y-2">
          <h3 className="text-sm font-medium text-muted-foreground mb-3">
            Jobs ({jobs.length})
          </h3>
          {jobs.map(job => (
            <JobItem key={job.id} job={job} />
          ))}
        </div>
      </ScrollArea>
    </div>
  );
}
