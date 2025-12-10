import { appState } from './state'
import { pubsub, RunEventPayload, RunCommandPayload } from './pubsub'
import type { DataLoaders } from './loaders'

interface GraphQLContext {
  loaders: DataLoaders
}

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

function formatRun(run: {
  id: string
  status: string
  workflowsDir: string
  startedAt: Date
  completedAt?: Date | null
  isPaused: boolean
  pausedAt?: Date | null
  currentWorkflow?: string | null
  currentJob?: string | null
  currentStep?: number | null
  events?: unknown[]
}) {
  return {
    id: run.id,
    status: run.status.toUpperCase(),
    workflowsDir: run.workflowsDir,
    startedAt: run.startedAt.toISOString(),
    completedAt: run.completedAt?.toISOString() ?? null,
    eventCount: run.events?.length ?? 0,
    isPaused: run.isPaused,
    pausedAt: run.pausedAt?.toISOString() ?? null,
    currentWorkflow: run.currentWorkflow ?? null,
    currentJob: run.currentJob ?? null,
    currentStep: run.currentStep ?? null
  }
}

export const resolvers = {
  Query: {
    health: () => 'ok',

    run: async (_: unknown, { id }: { id: string }, context: GraphQLContext) => {
      await appState.init()

      // Try memory cache first (has events), fall back to dataloader for DB
      const cachedRun = appState.getRun(id)
      if (cachedRun) {
        return formatRun(cachedRun)
      }

      // Use dataloader for batched DB access
      const dbRun = await context.loaders.runLoader.load(id)
      if (!dbRun) return null

      return formatRun({ ...dbRun, events: [] })
    },

    runs: async (_: unknown, { limit, offset }: { limit: number; offset: number }, context: GraphQLContext) => {
      await appState.init()

      // Get runs from memory cache (has events and real-time updates)
      const runs = appState.listRuns(limit, offset)

      // Prime the dataloader cache with these runs
      for (const run of runs) {
        context.loaders.runLoader.prime(run.id, {
          id: run.id,
          status: run.status,
          workflowsDir: run.workflowsDir,
          startedAt: run.startedAt,
          completedAt: run.completedAt ?? null,
          agentToken: run.agentToken ?? null,
          isPaused: run.isPaused,
          pausedAt: run.pausedAt ?? null,
          currentWorkflow: run.currentWorkflow ?? null,
          currentJob: run.currentJob ?? null,
          currentStep: run.currentStep ?? null
        })
      }

      return runs.map(formatRun)
    },

    runEvents: async (_: unknown, { runId }: { runId: string }) => {
      await appState.init()
      const run = appState.getRun(runId)
      return run?.events ?? []
    }
  },

  Mutation: {
    registerRun: async (_: unknown, { input }: { input: RegisterRunInput }, context: GraphQLContext) => {
      await appState.init()
      const startedAt = new Date(input.startedAt)
      const run = await appState.registerRun(input.runId, input.workflowsDir, startedAt, input.agentToken)

      // Clear dataloader cache for this run (new data)
      context.loaders.runLoader.clear(input.runId)

      const event: RunEventPayload = {
        eventType: 'RUN_STARTED',
        runId: input.runId,
        timestamp: startedAt.toISOString()
      }
      pubsub.publish('events', event)
      pubsub.publish('eventsForRun', input.runId, event)

      return formatRun(run)
    },

    reportEvents: async (_: unknown, { events }: { events: RunEventPayload[] }, context: GraphQLContext) => {
      await appState.init()

      // Clear dataloader cache for affected runs
      const affectedRunIds = new Set(events.map(e => e.runId))
      for (const runId of affectedRunIds) {
        context.loaders.runLoader.clear(runId)
      }

      for (const event of events) {
        pubsub.publish('events', event)
        pubsub.publish('eventsForRun', event.runId, event)
      }

      return appState.addEvents(events)
    },

    completeRun: async (_: unknown, { input }: { input: CompleteRunInput }, context: GraphQLContext) => {
      await appState.init()
      const completedAt = new Date(input.completedAt)
      await appState.completeRun(input.runId, input.success, completedAt)

      // Clear dataloader cache
      context.loaders.runLoader.clear(input.runId)

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

    cancelRun: async (_: unknown, { runId }: { runId: string }, context: GraphQLContext) => {
      await appState.init()
      await appState.cancelRun(runId)

      // Clear dataloader cache
      context.loaders.runLoader.clear(runId)

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

    pauseRun: async (_: unknown, { runId }: { runId: string }, context: GraphQLContext) => {
      await appState.init()

      const agentToken = await appState.getAgentToken(runId)
      if (!agentToken) {
        throw new Error('Run not found or no agent token registered')
      }

      const success = await appState.pauseRun(runId)
      if (!success) {
        throw new Error('Run is not running or already paused')
      }

      // Clear dataloader cache
      context.loaders.runLoader.clear(runId)

      const command: RunCommandPayload = {
        commandType: 'PAUSE',
        runId,
        timestamp: new Date().toISOString(),
        agentToken
      }
      pubsub.publish('commandsForRun', runId, command)

      return true
    },

    resumeRun: async (_: unknown, { runId }: { runId: string }, context: GraphQLContext) => {
      await appState.init()

      const agentToken = await appState.getAgentToken(runId)
      if (!agentToken) {
        throw new Error('Run not found or no agent token registered')
      }

      const success = await appState.resumeRun(runId)
      if (!success) {
        throw new Error('Run is not paused')
      }

      // Clear dataloader cache
      context.loaders.runLoader.clear(runId)

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
