import { appState } from './state'
import { pubsub, RunEventPayload, RunCommandPayload } from './pubsub'

interface RegisterRunInput {
  runId: string
  workflowsDir: string
  startedAt: string
  agentToken: string
}

interface CompleteRunInput {
  runId: string
  success: boolean
  completedAt: string
}

export const resolvers = {
  Query: {
    health: () => 'ok',

    run: async (_: unknown, { id }: { id: string }) => {
      await appState.init()
      const run = appState.getRun(id)
      if (!run) return null
      return {
        id: run.id,
        status: run.status.toUpperCase(),
        workflowsDir: run.workflowsDir,
        startedAt: run.startedAt.toISOString(),
        completedAt: run.completedAt?.toISOString() ?? null,
        eventCount: run.events.length,
        isPaused: run.isPaused,
        pausedAt: run.pausedAt?.toISOString() ?? null,
        currentWorkflow: run.currentWorkflow ?? null,
        currentJob: run.currentJob ?? null,
        currentStep: run.currentStep ?? null
      }
    },

    runs: async (_: unknown, { limit, offset }: { limit: number; offset: number }) => {
      await appState.init()
      return appState.listRuns(limit, offset).map(run => ({
        id: run.id,
        status: run.status.toUpperCase(),
        workflowsDir: run.workflowsDir,
        startedAt: run.startedAt.toISOString(),
        completedAt: run.completedAt?.toISOString() ?? null,
        eventCount: run.events.length,
        isPaused: run.isPaused,
        pausedAt: run.pausedAt?.toISOString() ?? null,
        currentWorkflow: run.currentWorkflow ?? null,
        currentJob: run.currentJob ?? null,
        currentStep: run.currentStep ?? null
      }))
    },

    runEvents: async (_: unknown, { runId }: { runId: string }) => {
      await appState.init()
      const run = appState.getRun(runId)
      return run?.events ?? []
    }
  },

  Mutation: {
    registerRun: async (_: unknown, { input }: { input: RegisterRunInput }) => {
      await appState.init()
      const startedAt = new Date(input.startedAt)
      const run = await appState.registerRun(input.runId, input.workflowsDir, startedAt, input.agentToken)

      const event: RunEventPayload = {
        eventType: 'RUN_STARTED',
        runId: input.runId,
        timestamp: startedAt.toISOString()
      }
      pubsub.publish('events', event)
      pubsub.publish('eventsForRun', input.runId, event)

      return {
        id: run.id,
        status: run.status.toUpperCase(),
        workflowsDir: run.workflowsDir,
        startedAt: run.startedAt.toISOString(),
        completedAt: run.completedAt?.toISOString() ?? null,
        eventCount: run.events.length,
        isPaused: run.isPaused,
        pausedAt: run.pausedAt?.toISOString() ?? null,
        currentWorkflow: run.currentWorkflow ?? null,
        currentJob: run.currentJob ?? null,
        currentStep: run.currentStep ?? null
      }
    },

    reportEvents: async (_: unknown, { events }: { events: RunEventPayload[] }) => {
      await appState.init()

      for (const event of events) {
        pubsub.publish('events', event)
        pubsub.publish('eventsForRun', event.runId, event)
      }

      return appState.addEvents(events)
    },

    completeRun: async (_: unknown, { input }: { input: CompleteRunInput }) => {
      await appState.init()
      const completedAt = new Date(input.completedAt)
      await appState.completeRun(input.runId, input.success, completedAt)

      const event: RunEventPayload = {
        eventType: 'RUN_COMPLETED',
        runId: input.runId,
        timestamp: completedAt.toISOString(),
        success: input.success
      }
      pubsub.publish('events', event)
      pubsub.publish('eventsForRun', input.runId, event)

      return true
    },

    cancelRun: async (_: unknown, { runId }: { runId: string }) => {
      await appState.init()
      await appState.cancelRun(runId)

      const event: RunEventPayload = {
        eventType: 'RUN_COMPLETED',
        runId,
        timestamp: new Date().toISOString(),
        success: false
      }
      pubsub.publish('events', event)
      pubsub.publish('eventsForRun', runId, event)

      return true
    },

    stopRun: async (_: unknown, { runId }: { runId: string }) => {
      await appState.init()

      const agentToken = await appState.getAgentToken(runId)
      if (!agentToken) {
        throw new Error('Run not found or no agent token registered')
      }

      const command: RunCommandPayload = {
        commandType: 'STOP',
        runId,
        timestamp: new Date().toISOString(),
        agentToken
      }
      pubsub.publish('commandsForRun', runId, command)

      return true
    },

    pauseRun: async (_: unknown, { runId }: { runId: string }) => {
      await appState.init()

      const agentToken = await appState.getAgentToken(runId)
      if (!agentToken) {
        throw new Error('Run not found or no agent token registered')
      }

      const success = await appState.pauseRun(runId)
      if (!success) {
        throw new Error('Run is not running or already paused')
      }

      const command: RunCommandPayload = {
        commandType: 'PAUSE',
        runId,
        timestamp: new Date().toISOString(),
        agentToken
      }
      pubsub.publish('commandsForRun', runId, command)

      return true
    },

    resumeRun: async (_: unknown, { runId }: { runId: string }) => {
      await appState.init()

      const agentToken = await appState.getAgentToken(runId)
      if (!agentToken) {
        throw new Error('Run not found or no agent token registered')
      }

      const success = await appState.resumeRun(runId)
      if (!success) {
        throw new Error('Run is not paused')
      }

      const command: RunCommandPayload = {
        commandType: 'RESUME',
        runId,
        timestamp: new Date().toISOString(),
        agentToken
      }
      pubsub.publish('commandsForRun', runId, command)

      return true
    }
  },

  Subscription: {
    events: {
      subscribe: () => pubsub.subscribe('events'),
      resolve: (payload: RunEventPayload) => payload
    },
    eventsForRun: {
      subscribe: (_: unknown, { runId }: { runId: string }) =>
        pubsub.subscribe('eventsForRun', runId),
      resolve: (payload: RunEventPayload) => payload
    },
    commandsForRun: {
      subscribe: (_: unknown, { runId }: { runId: string }) =>
        pubsub.subscribe('commandsForRun', runId),
      resolve: (payload: RunCommandPayload) => payload
    }
  }
}
