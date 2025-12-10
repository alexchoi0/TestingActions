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

// Mock data for demo (using UUID-style IDs)
const mockWorkflows: Record<string, Workflow> = {
  setup: {
    id: 'a1b2c3d4-e5f6-4a5b-8c9d-0e1f2a3b4c5d',
    name: 'setup',
    status: 'success',
    dependsOn: [],
    jobs: {
      init: {
        id: 'b2c3d4e5-f6a7-4b5c-9d0e-1f2a3b4c5d6e',
        name: 'Initialize Database',
        status: 'success',
        steps: [
          { id: 'c3d4e5f6-a7b8-4c5d-0e1f-2a3b4c5d6e7f', name: 'Start PostgreSQL', uses: 'bash/exec', status: 'success' },
          { id: 'd4e5f6a7-b8c9-4d5e-1f2a-3b4c5d6e7f8a', name: 'Run migrations', uses: 'bash/exec', status: 'success' },
          { id: 'e5f6a7b8-c9d0-4e5f-2a3b-4c5d6e7f8a9b', name: 'Seed test data', uses: 'bash/exec', status: 'success' },
        ],
      },
    },
  },
  'api-tests': {
    id: 'f6a7b8c9-d0e1-4f5a-3b4c-5d6e7f8a9b0c',
    name: 'api-tests',
    status: 'running',
    dependsOn: ['setup'],
    jobs: {
      test: {
        id: 'a7b8c9d0-e1f2-4a5b-4c5d-6e7f8a9b0c1d',
        name: 'API Tests',
        status: 'running',
        steps: [
          { id: 'b8c9d0e1-f2a3-4b5c-5d6e-7f8a9b0c1d2e', name: 'Health check', uses: 'web/get', status: 'success' },
          { id: 'c9d0e1f2-a3b4-4c5d-6e7f-8a9b0c1d2e3f', name: 'Auth endpoints', uses: 'web/post', status: 'running' },
          { id: 'd0e1f2a3-b4c5-4d5e-7f8a-9b0c1d2e3f4a', name: 'User CRUD', uses: 'web/get', status: 'pending' },
        ],
      },
    },
  },
  'e2e-tests': {
    id: 'e1f2a3b4-c5d6-4e5f-8a9b-0c1d2e3f4a5b',
    name: 'e2e-tests',
    status: 'running',
    dependsOn: ['setup'],
    jobs: {
      chrome: {
        id: 'f2a3b4c5-d6e7-4f5a-9b0c-1d2e3f4a5b6c',
        name: 'Chrome Tests',
        status: 'success',
        steps: [
          { id: 'a3b4c5d6-e7f8-4a5b-0c1d-2e3f4a5b6c7d', name: 'Login flow', uses: 'page/goto', status: 'success' },
          { id: 'b4c5d6e7-f8a9-4b5c-1d2e-3f4a5b6c7d8e', name: 'Dashboard', uses: 'element/click', status: 'success' },
        ],
      },
      firefox: {
        id: 'c5d6e7f8-a9b0-4c5d-2e3f-4a5b6c7d8e9f',
        name: 'Firefox Tests',
        status: 'running',
        steps: [
          { id: 'd6e7f8a9-b0c1-4d5e-3f4a-5b6c7d8e9f0a', name: 'Login flow', uses: 'page/goto', status: 'success' },
          { id: 'e7f8a9b0-c1d2-4e5f-4a5b-6c7d8e9f0a1b', name: 'Dashboard', uses: 'element/click', status: 'running' },
        ],
      },
    },
  },
  cleanup: {
    id: 'f8a9b0c1-d2e3-4f5a-5b6c-7d8e9f0a1b2c',
    name: 'cleanup',
    status: 'pending',
    dependsOn: ['api-tests', 'e2e-tests'],
    jobs: {
      teardown: {
        id: 'a9b0c1d2-e3f4-4a5b-6c7d-8e9f0a1b2c3d',
        name: 'Teardown',
        status: 'pending',
        steps: [
          { id: 'b0c1d2e3-f4a5-4b5c-7d8e-9f0a1b2c3d4e', name: 'Stop containers', uses: 'bash/exec', status: 'pending' },
          { id: 'c1d2e3f4-a5b6-4c5d-8e9f-0a1b2c3d4e5f', name: 'Cleanup artifacts', uses: 'bash/exec', status: 'pending' },
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
