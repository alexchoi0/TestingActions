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
  isPaused: boolean
  pausedAt?: Date
  currentWorkflow?: string
  currentJob?: string
  currentStep?: number
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
        events: [],
        isPaused: run.isPaused,
        pausedAt: run.pausedAt ?? undefined,
        currentWorkflow: run.currentWorkflow ?? undefined,
        currentJob: run.currentJob ?? undefined,
        currentStep: run.currentStep ?? undefined
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
      agentToken,
      isPaused: false
    }

    this.runs.set(runId, runInfo)

    await prisma.run.create({
      data: {
        id: runId,
        status: 'pending',
        workflowsDir,
        startedAt,
        agentToken,
        isPaused: false
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
        if (event.workflowName) {
          run.currentWorkflow = event.workflowName
        }
        if (event.jobName) {
          run.currentJob = event.jobName
        }
        if (event.stepIndex !== undefined && event.stepIndex !== null) {
          run.currentStep = event.stepIndex
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

  async pauseRun(runId: string): Promise<boolean> {
    const run = this.runs.get(runId)
    if (!run || run.status !== 'running') {
      return false
    }

    const pausedAt = new Date()
    run.isPaused = true
    run.pausedAt = pausedAt
    run.status = 'paused'

    await prisma.run.update({
      where: { id: runId },
      data: {
        isPaused: true,
        pausedAt,
        status: 'paused',
        currentWorkflow: run.currentWorkflow,
        currentJob: run.currentJob,
        currentStep: run.currentStep
      }
    })

    return true
  }

  async resumeRun(runId: string): Promise<boolean> {
    const run = this.runs.get(runId)
    if (!run || !run.isPaused) {
      return false
    }

    run.isPaused = false
    run.pausedAt = undefined
    run.status = 'running'

    await prisma.run.update({
      where: { id: runId },
      data: {
        isPaused: false,
        pausedAt: null,
        status: 'running'
      }
    })

    return true
  }
}

export const appState = new AppState()
