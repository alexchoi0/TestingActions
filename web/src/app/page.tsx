'use client';

import { useState, useCallback, useEffect, useMemo } from 'react';
import { useRouter } from 'next/navigation';
import dynamic from 'next/dynamic';
import RunControls from '@/components/workflow/run-controls';
import WorkflowDetail from '@/components/workflow/workflow-detail';
import { useSession } from '@/lib/auth-client';
import type { Workflow, RunResult } from '@/types/workflow';

const WorkflowDAG = dynamic(() => import('@/components/workflow/workflow-dag'), {
  ssr: false,
  loading: () => (
    <div className="flex items-center justify-center h-full text-muted-foreground">
      Loading DAG...
    </div>
  ),
});

// Mock data for demo
const mockWorkflows: Record<string, Workflow> = {
  setup: {
    id: 'setup',
    name: 'setup',
    status: 'success',
    dependsOn: [],
    jobs: {
      init: {
        id: 'init',
        name: 'Initialize Database',
        status: 'success',
        steps: [
          { id: 's1', name: 'Start PostgreSQL', uses: 'bash/exec', status: 'success' },
          { id: 's2', name: 'Run migrations', uses: 'bash/exec', status: 'success' },
          { id: 's3', name: 'Seed test data', uses: 'bash/exec', status: 'success' },
        ],
      },
    },
  },
  'api-tests': {
    id: 'api-tests',
    name: 'api-tests',
    status: 'running',
    dependsOn: ['setup'],
    jobs: {
      test: {
        id: 'test',
        name: 'API Tests',
        status: 'running',
        steps: [
          { id: 's1', name: 'Health check', uses: 'web/get', status: 'success' },
          { id: 's2', name: 'Auth endpoints', uses: 'web/post', status: 'running' },
          { id: 's3', name: 'User CRUD', uses: 'web/get', status: 'pending' },
        ],
      },
    },
  },
  'e2e-tests': {
    id: 'e2e-tests',
    name: 'e2e-tests',
    status: 'running',
    dependsOn: ['setup'],
    jobs: {
      chrome: {
        id: 'chrome',
        name: 'Chrome Tests',
        status: 'success',
        steps: [
          { id: 's1', name: 'Login flow', uses: 'page/goto', status: 'success' },
          { id: 's2', name: 'Dashboard', uses: 'element/click', status: 'success' },
        ],
      },
      firefox: {
        id: 'firefox',
        name: 'Firefox Tests',
        status: 'running',
        steps: [
          { id: 's1', name: 'Login flow', uses: 'page/goto', status: 'success' },
          { id: 's2', name: 'Dashboard', uses: 'element/click', status: 'running' },
        ],
      },
    },
  },
  cleanup: {
    id: 'cleanup',
    name: 'cleanup',
    status: 'pending',
    dependsOn: ['api-tests', 'e2e-tests'],
    jobs: {
      teardown: {
        id: 'teardown',
        name: 'Teardown',
        status: 'pending',
        steps: [
          { id: 's1', name: 'Stop containers', uses: 'bash/exec', status: 'pending' },
          { id: 's2', name: 'Cleanup artifacts', uses: 'bash/exec', status: 'pending' },
        ],
      },
    },
  },
};

function generateUUID(): string {
  return 'xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx'.replace(/[xy]/g, (c) => {
    const r = Math.random() * 16 | 0;
    const v = c === 'x' ? r : (r & 0x3 | 0x8);
    return v.toString(16);
  });
}

export default function Home() {
  const router = useRouter();
  const { data: session, isPending } = useSession();
  const [workflows, setWorkflows] = useState<Record<string, Workflow>>(mockWorkflows);
  const [selectedWorkflow, setSelectedWorkflow] = useState<string | null>(null);
  const [isRunning, setIsRunning] = useState(true);
  const [isPaused, setIsPaused] = useState(false);
  const runId = useMemo(() => generateUUID(), []);

  useEffect(() => {
    if (!isPending && !session) {
      router.push('/login');
    }
  }, [session, isPending, router]);

  if (isPending) {
    return (
      <div className="h-screen flex items-center justify-center bg-background">
        <p className="text-muted-foreground">Loading...</p>
      </div>
    );
  }

  if (!session) {
    return null;
  }

  const handleWorkflowSelect = useCallback((workflowId: string) => {
    setSelectedWorkflow(workflowId);
  }, []);

  const handleRun = useCallback(() => {
    setIsRunning(true);
    setIsPaused(false);
    // TODO: Connect to backend API
  }, []);

  const handleStop = useCallback(() => {
    setIsRunning(false);
    setIsPaused(false);
    // TODO: Connect to backend API
  }, []);

  const handlePause = useCallback(async () => {
    try {
      const response = await fetch('/graphql', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          query: `mutation { pauseRun(runId: "${runId}") }`
        })
      });
      const data = await response.json();
      if (data.data?.pauseRun) {
        setIsPaused(true);
      }
    } catch (error) {
      console.error('Failed to pause run:', error);
    }
  }, [runId]);

  const handleResume = useCallback(async () => {
    try {
      const response = await fetch('/graphql', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          query: `mutation { resumeRun(runId: "${runId}") }`
        })
      });
      const data = await response.json();
      if (data.data?.resumeRun) {
        setIsPaused(false);
      }
    } catch (error) {
      console.error('Failed to resume run:', error);
    }
  }, [runId]);

  return (
    <div className="h-screen flex flex-col bg-background">
      <RunControls
        isRunning={isRunning}
        isPaused={isPaused}
        runId={runId}
        onRun={handleRun}
        onStop={handleStop}
        onPause={handlePause}
        onResume={handleResume}
      />
      <div className="flex-1 flex overflow-hidden">
        <div className="flex-1 border-r">
          <WorkflowDAG
            workflows={workflows}
            onWorkflowSelect={handleWorkflowSelect}
          />
        </div>
        <div className="w-[400px] bg-muted/30">
          <WorkflowDetail
            workflow={selectedWorkflow ? workflows[selectedWorkflow] : null}
          />
        </div>
      </div>
    </div>
  );
}
