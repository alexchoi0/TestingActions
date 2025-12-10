export const typeDefs = /* GraphQL */ `
  enum Status {
    PENDING
    RUNNING
    PAUSED
    SUCCESS
    FAILED
    CANCELLED
  }

  enum EventType {
    RUN_STARTED
    RUN_COMPLETED
    WORKFLOW_STARTED
    WORKFLOW_COMPLETED
    WORKFLOW_SKIPPED
    JOB_STARTED
    JOB_COMPLETED
    STEP_STARTED
    STEP_COMPLETED
  }

  enum CommandType {
    STOP
    PAUSE
    RESUME
  }

  type Run {
    id: ID!
    status: Status!
    workflowsDir: String!
    startedAt: String!
    completedAt: String
    eventCount: Int!
    isPaused: Boolean!
    pausedAt: String
    currentWorkflow: String
    currentJob: String
    currentStep: Int
  }

  type RunEvent {
    eventType: EventType!
    runId: String!
    timestamp: String!
    workflowName: String
    jobName: String
    stepIndex: Int
    stepName: String
    success: Boolean
    error: String
    reason: String
  }

  type RunCommand {
    commandType: CommandType!
    runId: String!
    timestamp: String!
    agentToken: String!
  }

  input RegisterRunInput {
    runId: String!
    workflowsDir: String!
    startedAt: String!
    agentToken: String!
  }

  input RunEventInput {
    eventType: EventType!
    runId: String!
    timestamp: String!
    workflowName: String
    jobName: String
    stepIndex: Int
    stepName: String
    success: Boolean
    error: String
    reason: String
  }

  input CompleteRunInput {
    runId: String!
    success: Boolean!
    completedAt: String!
  }

  type Query {
    health: String!
    run(id: ID!): Run
    runs(limit: Int = 20, offset: Int = 0): [Run!]!
    runEvents(runId: ID!): [RunEvent!]!
  }

  type Mutation {
    registerRun(input: RegisterRunInput!): Run!
    reportEvents(events: [RunEventInput!]!): Int!
    completeRun(input: CompleteRunInput!): Boolean!
    cancelRun(runId: ID!): Boolean!
    stopRun(runId: ID!): Boolean!
    pauseRun(runId: ID!): Boolean!
    resumeRun(runId: ID!): Boolean!
  }

  type Subscription {
    events: RunEvent!
    eventsForRun(runId: ID!): RunEvent!
    commandsForRun(runId: ID!): RunCommand!
  }
`
