import { createPubSub } from 'graphql-yoga'

export interface RunEventPayload {
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

export type CommandType = 'STOP' | 'PAUSE' | 'RESUME'

export interface RunCommandPayload {
  commandType: CommandType
  runId: string
  timestamp: string
  agentToken: string
}

export const pubsub = createPubSub<{
  events: [RunEventPayload]
  eventsForRun: [runId: string, RunEventPayload]
  commandsForRun: [runId: string, RunCommandPayload]
}>()
