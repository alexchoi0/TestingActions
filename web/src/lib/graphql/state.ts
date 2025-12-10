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

interface RunInfo {
  id: string
  status: string
  workflowsDir: string
  startedAt: Date
  completedAt?: Date
  events: RunEvent[]
  agentToken?: string
}

class AppState {
  private runs = new Map<string, RunInfo>()
  private initialized = false

  async init() {
    if (this.initialized) return

    const dbRuns = await prisma.run.findMany({
      orderBy: { startedAt: 'desc' },
      take: 100
    })

    for (const run of dbRuns) {
      this.runs.set(run.id, {
        id: run.id,
        status: run.status,
        workflowsDir: run.workflowsDir,
        startedAt: run.startedAt,
        completedAt: run.completedAt ?? undefined,
        events: []
      })
    }

    this.initialized = true
  }

  async registerRun(runId: string, workflowsDir: string, startedAt: Date, agentToken: string) {
    const runInfo: RunInfo = {
      id: runId,
      status: 'pending',
      workflowsDir,
      startedAt,
      events: [],
      agentToken
    }

    this.runs.set(runId, runInfo)

    await prisma.run.create({
      data: {
        id: runId,
        status: 'pending',
        workflowsDir,
        startedAt,
        agentToken
      }
    })

    return runInfo
  }

  async getAgentToken(runId: string): Promise<string | undefined> {
    const cached = this.runs.get(runId)?.agentToken
    if (cached) return cached

    const run = await prisma.run.findUnique({
      where: { id: runId },
      select: { agentToken: true }
    })
    return run?.agentToken ?? undefined
  }

  addEvents(events: RunEvent[]): number {
    for (const event of events) {
      const run = this.runs.get(event.runId)
      if (run) {
        if (event.eventType === 'RUN_STARTED') {
          run.status = 'running'
        }
        run.events.push(event)
      }
    }
    return events.length
  }

  async completeRun(runId: string, success: boolean, completedAt: Date) {
    const status = success ? 'success' : 'failed'
    const run = this.runs.get(runId)

    if (run) {
      run.status = status
      run.completedAt = completedAt
    }

    await prisma.run.update({
      where: { id: runId },
      data: { status, completedAt }
    })
  }

  async cancelRun(runId: string) {
    const run = this.runs.get(runId)
    if (run) {
      run.status = 'cancelled'
      run.completedAt = new Date()
    }

    await prisma.run.update({
      where: { id: runId },
      data: { status: 'cancelled', completedAt: new Date() }
    })
  }

  getRun(runId: string): RunInfo | undefined {
    return this.runs.get(runId)
  }

  listRuns(limit: number, offset: number): RunInfo[] {
    const sorted = [...this.runs.values()]
      .sort((a, b) => b.startedAt.getTime() - a.startedAt.getTime())
    return sorted.slice(offset, offset + limit)
  }
}

export const appState = new AppState()
