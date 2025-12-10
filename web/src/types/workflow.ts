export type WorkflowStatus = 'pending' | 'running' | 'paused' | 'success' | 'failed' | 'skipped';

export type JobStatus = 'pending' | 'running' | 'success' | 'failed' | 'skipped';

export type StepStatus = 'pending' | 'running' | 'success' | 'failed' | 'skipped';

export interface Step {
  id: string;
  name: string;
  uses: string;
  status: StepStatus;
  startedAt?: string;
  completedAt?: string;
  error?: string;
  outputs?: Record<string, string>;
}

export interface Job {
  id: string;
  name: string;
  status: JobStatus;
  steps: Step[];
  startedAt?: string;
  completedAt?: string;
}

export interface Workflow {
  id: string;
  name: string;
  status: WorkflowStatus;
  dependsOn: string[];
  jobs: Record<string, Job>;
  startedAt?: string;
  completedAt?: string;
}

export interface RunResult {
  runId: string;
  success: boolean;
  workflows: Record<string, Workflow>;
  startedAt: string;
  completedAt?: string;
}
