import DataLoader from 'dataloader'
import { prisma } from '../db'

interface RunEvent {
  eventType: string
  runId: string
  timestamp: string
  workflowName?: string | null
  jobName?: string | null
  stepIndex?: number | null
  stepName?: string | null
  success?: boolean | null
  error?: string | null
  reason?: string | null
}

interface RunData {
  id: string
  status: string
  workflowsDir: string
  startedAt: Date
  completedAt: Date | null
  agentToken: string | null
  isPaused: boolean
  pausedAt: Date | null
  currentWorkflow: string | null
  currentJob: string | null
  currentStep: number | null
}

export interface DataLoaders {
  runLoader: DataLoader<string, RunData | null>
  runEventsLoader: DataLoader<string, RunEvent[]>
}

export function createLoaders(): DataLoaders {
  const runLoader = new DataLoader<string, RunData | null>(
    async (ids) => {
      const runs = await prisma.run.findMany({
        where: { id: { in: [...ids] } }
      })

      const runMap = new Map(runs.map(run => [run.id, run]))
      return ids.map(id => runMap.get(id) ?? null)
    },
    { cache: true }
  )

  const runEventsLoader = new DataLoader<string, RunEvent[]>(
    async (runIds) => {
      // For now, events are stored in memory in appState
      // This loader is prepared for when events move to the database
      // Currently returns empty arrays as events are in-memory
      return runIds.map(() => [])
    },
    { cache: true }
  )

  return {
    runLoader,
    runEventsLoader
  }
}
