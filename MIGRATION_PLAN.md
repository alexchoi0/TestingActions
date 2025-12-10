# Rust Web Server to Next.js Migration Plan

## Overview

Migrate the Rust telemetry server (`testing-actions-server`) into the existing Next.js application, consolidating all web functionality into a single service.

**Current Architecture:**
- Rust server on port 3001: GraphQL API + WebSocket subscriptions
- Next.js on port 3000: Frontend UI (currently uses mock data)

**Target Architecture:**
- Next.js on port 3000: GraphQL API + WebSocket + Frontend UI

---

## Phase 1: Database Layer Setup

### 1.1 Install Dependencies

```bash
cd web
npm install prisma @prisma/client graphql graphql-yoga ws @graphql-yoga/subscription
npm install -D prisma
```

### 1.2 Create Prisma Schema

Create `web/prisma/schema.prisma`:

```prisma
generator client {
  provider = "prisma-client-js"
}

datasource db {
  provider = "sqlite"  // Can be changed to postgresql/mysql
  url      = env("DATABASE_URL")
}

model Run {
  id           String    @id
  status       String    @default("pending")
  workflowsDir String    @map("workflows_dir")
  startedAt    DateTime  @map("started_at")
  completedAt  DateTime? @map("completed_at")
  configJson   String?   @map("config_json")

  workflowResults WorkflowResult[]
  jobResults      JobResult[]
  stepResults     StepResult[]
  artifacts       Artifact[]

  @@map("runs")
}

model WorkflowResult {
  id           String    @id @default(uuid())
  runId        String    @map("run_id")
  workflowName String    @map("workflow_name")
  status       String
  startedAt    DateTime  @map("started_at")
  completedAt  DateTime? @map("completed_at")
  error        String?

  run Run @relation(fields: [runId], references: [id])

  @@map("workflow_results")
}

model JobResult {
  id           String    @id @default(uuid())
  runId        String    @map("run_id")
  workflowName String    @map("workflow_name")
  jobName      String    @map("job_name")
  status       String
  startedAt    DateTime  @map("started_at")
  completedAt  DateTime? @map("completed_at")
  error        String?

  run Run @relation(fields: [runId], references: [id])

  @@map("job_results")
}

model StepResult {
  id           String    @id @default(uuid())
  runId        String    @map("run_id")
  workflowName String    @map("workflow_name")
  jobName      String    @map("job_name")
  stepIndex    Int       @map("step_index")
  stepName     String    @map("step_name")
  status       String
  startedAt    DateTime  @map("started_at")
  completedAt  DateTime? @map("completed_at")
  outputJson   String?   @map("output_json")
  error        String?

  run Run @relation(fields: [runId], references: [id])

  @@map("step_results")
}

model Artifact {
  id          String   @id @default(uuid())
  runId       String   @map("run_id")
  workflowName String  @map("workflow_name")
  jobName     String   @map("job_name")
  name        String
  path        String
  sizeBytes   Int      @map("size_bytes")
  contentType String?  @map("content_type")
  createdAt   DateTime @map("created_at")

  run Run @relation(fields: [runId], references: [id])

  @@map("artifacts")
}
```

### 1.3 Create Database Client

Create `web/src/lib/db.ts`:

```typescript
import { PrismaClient } from '@prisma/client'

const globalForPrisma = globalThis as unknown as { prisma: PrismaClient }

export const prisma = globalForPrisma.prisma || new PrismaClient()

if (process.env.NODE_ENV !== 'production') globalForPrisma.prisma = prisma
```

---

## Phase 2: GraphQL API Implementation

### 2.1 Create Type Definitions

Create `web/src/lib/graphql/types.ts`:

```typescript
export const typeDefs = /* GraphQL */ `
  enum Status {
    PENDING
    RUNNING
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

  type Run {
    id: ID!
    status: Status!
    workflowsDir: String!
    startedAt: String!
    completedAt: String
    eventCount: Int!
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

  input RegisterRunInput {
    runId: String!
    workflowsDir: String!
    startedAt: String!
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
  }

  type Subscription {
    events: RunEvent!
    eventsForRun(runId: ID!): RunEvent!
  }
`
```

### 2.2 Create Event Broadcaster

Create `web/src/lib/graphql/pubsub.ts`:

```typescript
import { createPubSub } from 'graphql-yoga'

export interface RunEventPayload {
  eventType: string
  runId: string
  timestamp: string
  workflowName?: string
  jobName?: string
  stepIndex?: number
  stepName?: string
  success?: boolean
  error?: string
  reason?: string
}

export const pubsub = createPubSub<{
  events: [RunEventPayload]
  eventsForRun: [runId: string, RunEventPayload]
}>()
```

### 2.3 Create In-Memory State Manager

Create `web/src/lib/graphql/state.ts`:

```typescript
import { prisma } from '../db'

interface RunEvent {
  eventType: string
  runId: string
  timestamp: string
  workflowName?: string
  jobName?: string
  stepIndex?: number
  stepName?: string
  success?: boolean
  error?: string
  reason?: string
}

interface RunInfo {
  id: string
  status: string
  workflowsDir: string
  startedAt: Date
  completedAt?: Date
  events: RunEvent[]
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

  async registerRun(runId: string, workflowsDir: string, startedAt: Date) {
    const runInfo: RunInfo = {
      id: runId,
      status: 'pending',
      workflowsDir,
      startedAt,
      events: []
    }

    this.runs.set(runId, runInfo)

    await prisma.run.create({
      data: {
        id: runId,
        status: 'pending',
        workflowsDir,
        startedAt
      }
    })

    return runInfo
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
```

### 2.4 Create Resolvers

Create `web/src/lib/graphql/resolvers.ts`:

```typescript
import { appState } from './state'
import { pubsub, RunEventPayload } from './pubsub'

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
        completedAt: run.completedAt?.toISOString(),
        eventCount: run.events.length
      }
    },

    runs: async (_: unknown, { limit, offset }: { limit: number; offset: number }) => {
      await appState.init()
      return appState.listRuns(limit, offset).map(run => ({
        id: run.id,
        status: run.status.toUpperCase(),
        workflowsDir: run.workflowsDir,
        startedAt: run.startedAt.toISOString(),
        completedAt: run.completedAt?.toISOString(),
        eventCount: run.events.length
      }))
    },

    runEvents: async (_: unknown, { runId }: { runId: string }) => {
      await appState.init()
      const run = appState.getRun(runId)
      return run?.events ?? []
    }
  },

  Mutation: {
    registerRun: async (_: unknown, { input }: { input: { runId: string; workflowsDir: string; startedAt: string } }) => {
      await appState.init()
      const startedAt = new Date(input.startedAt)
      const run = await appState.registerRun(input.runId, input.workflowsDir, startedAt)

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
        completedAt: run.completedAt?.toISOString(),
        eventCount: run.events.length
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

    completeRun: async (_: unknown, { input }: { input: { runId: string; success: boolean; completedAt: string } }) => {
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
    }
  }
}
```

### 2.5 Create GraphQL Route Handler

Create `web/src/app/graphql/route.ts`:

```typescript
import { createYoga, createSchema } from 'graphql-yoga'
import { typeDefs } from '@/lib/graphql/types'
import { resolvers } from '@/lib/graphql/resolvers'

const yoga = createYoga({
  schema: createSchema({ typeDefs, resolvers }),
  graphqlEndpoint: '/graphql',
  fetchAPI: { Response }
})

export { yoga as GET, yoga as POST }
```

---

## Phase 3: WebSocket Subscriptions

### 3.1 Create Custom Server

Create `web/server.ts`:

```typescript
import { createServer } from 'http'
import { parse } from 'url'
import next from 'next'
import { WebSocketServer } from 'ws'
import { useServer } from 'graphql-ws/lib/use/ws'
import { createYoga, createSchema } from 'graphql-yoga'
import { typeDefs } from './src/lib/graphql/types'
import { resolvers } from './src/lib/graphql/resolvers'

const dev = process.env.NODE_ENV !== 'production'
const hostname = 'localhost'
const port = parseInt(process.env.PORT || '3000', 10)

const app = next({ dev, hostname, port })
const handle = app.getRequestHandler()

const schema = createSchema({ typeDefs, resolvers })

app.prepare().then(() => {
  const server = createServer((req, res) => {
    const parsedUrl = parse(req.url!, true)
    handle(req, res, parsedUrl)
  })

  const wsServer = new WebSocketServer({
    server,
    path: '/graphql/ws'
  })

  useServer({ schema }, wsServer)

  server.listen(port, () => {
    console.log(`> Ready on http://${hostname}:${port}`)
    console.log(`> WebSocket subscriptions on ws://${hostname}:${port}/graphql/ws`)
  })
})
```

### 3.2 Update package.json Scripts

```json
{
  "scripts": {
    "dev": "ts-node --project tsconfig.server.json server.ts",
    "build": "next build",
    "start": "NODE_ENV=production ts-node --project tsconfig.server.json server.ts"
  }
}
```

---

## Phase 4: Frontend Integration

### 4.1 Install GraphQL Client

```bash
npm install @apollo/client graphql-ws
```

### 4.2 Create Apollo Provider

Create `web/src/lib/apollo.tsx`:

```typescript
'use client'

import { ApolloClient, InMemoryCache, ApolloProvider, split, HttpLink } from '@apollo/client'
import { GraphQLWsLink } from '@apollo/client/link/subscriptions'
import { getMainDefinition } from '@apollo/client/utilities'
import { createClient } from 'graphql-ws'

const httpLink = new HttpLink({
  uri: '/graphql'
})

const wsLink = typeof window !== 'undefined'
  ? new GraphQLWsLink(createClient({
      url: `ws://${window.location.host}/graphql/ws`
    }))
  : null

const splitLink = typeof window !== 'undefined' && wsLink
  ? split(
      ({ query }) => {
        const definition = getMainDefinition(query)
        return definition.kind === 'OperationDefinition' && definition.operation === 'subscription'
      },
      wsLink,
      httpLink
    )
  : httpLink

const client = new ApolloClient({
  link: splitLink,
  cache: new InMemoryCache()
})

export function GraphQLProvider({ children }: { children: React.ReactNode }) {
  return <ApolloProvider client={client}>{children}</ApolloProvider>
}
```

### 4.3 Update Layout

Update `web/src/app/layout.tsx` to wrap with GraphQLProvider.

### 4.4 Create GraphQL Hooks

Create `web/src/hooks/useRuns.ts`:

```typescript
import { gql, useQuery, useSubscription } from '@apollo/client'

const RUNS_QUERY = gql`
  query Runs($limit: Int, $offset: Int) {
    runs(limit: $limit, offset: $offset) {
      id
      status
      workflowsDir
      startedAt
      completedAt
      eventCount
    }
  }
`

const EVENTS_SUBSCRIPTION = gql`
  subscription Events {
    events {
      eventType
      runId
      timestamp
      workflowName
      jobName
      stepIndex
      stepName
      success
      error
    }
  }
`

export function useRuns(limit = 20, offset = 0) {
  const { data, loading, error, refetch } = useQuery(RUNS_QUERY, {
    variables: { limit, offset }
  })

  useSubscription(EVENTS_SUBSCRIPTION, {
    onData: () => refetch()
  })

  return { runs: data?.runs ?? [], loading, error }
}
```

---

## Phase 5: Health Endpoint

Create `web/src/app/health/route.ts`:

```typescript
export function GET() {
  return new Response('ok', { status: 200 })
}
```

---

## Phase 6: Configuration

### 6.1 Environment Variables

Create `web/.env.example`:

```env
DATABASE_URL="file:./dev.db"
# For PostgreSQL: DATABASE_URL="postgresql://user:pass@localhost:5432/testing_actions"
# For MySQL: DATABASE_URL="mysql://user:pass@localhost:3306/testing_actions"
```

### 6.2 Update .gitignore

Add to `web/.gitignore`:

```
*.db
*.db-journal
```

---

## Phase 7: Testing & Validation

### 7.1 Verify GraphQL Playground

Navigate to `http://localhost:3000/graphql` to access the interactive playground.

### 7.2 Test Agent Compatibility

Update the Rust agent's default server URL from `http://localhost:3001` to `http://localhost:3000` or make it configurable.

### 7.3 Test Queries

```graphql
query {
  health
  runs(limit: 10) {
    id
    status
    startedAt
  }
}
```

### 7.4 Test Mutations

```graphql
mutation {
  registerRun(input: {
    runId: "test-run-1"
    workflowsDir: "/path/to/workflows"
    startedAt: "2024-01-01T00:00:00Z"
  }) {
    id
    status
  }
}
```

### 7.5 Test Subscriptions

```graphql
subscription {
  events {
    eventType
    runId
    timestamp
  }
}
```

---

## Migration Checklist

- [ ] Install dependencies (Phase 1.1)
- [ ] Create Prisma schema (Phase 1.2)
- [ ] Run `npx prisma generate` and `npx prisma db push`
- [ ] Create database client (Phase 1.3)
- [ ] Create GraphQL types (Phase 2.1)
- [ ] Create PubSub (Phase 2.2)
- [ ] Create state manager (Phase 2.3)
- [ ] Create resolvers (Phase 2.4)
- [ ] Create GraphQL route (Phase 2.5)
- [ ] Create custom server for WebSocket (Phase 3.1)
- [ ] Update package.json scripts (Phase 3.2)
- [ ] Install Apollo Client (Phase 4.1)
- [ ] Create Apollo provider (Phase 4.2)
- [ ] Update layout (Phase 4.3)
- [ ] Create GraphQL hooks (Phase 4.4)
- [ ] Create health endpoint (Phase 5)
- [ ] Configure environment variables (Phase 6)
- [ ] Test all endpoints (Phase 7)
- [ ] Update agent default URL
- [ ] Remove Rust server dependency from deployment

---

## File Structure After Migration

```
web/
├── prisma/
│   └── schema.prisma
├── src/
│   ├── app/
│   │   ├── graphql/
│   │   │   └── route.ts
│   │   ├── health/
│   │   │   └── route.ts
│   │   ├── layout.tsx
│   │   └── page.tsx
│   ├── components/
│   │   └── ...
│   ├── hooks/
│   │   └── useRuns.ts
│   ├── lib/
│   │   ├── apollo.tsx
│   │   ├── db.ts
│   │   └── graphql/
│   │       ├── types.ts
│   │       ├── resolvers.ts
│   │       ├── state.ts
│   │       └── pubsub.ts
│   └── types/
│       └── workflow.ts
├── server.ts
├── package.json
└── .env
```

---

## Notes

1. **Database Flexibility**: Prisma supports SQLite, PostgreSQL, and MySQL. Change the `provider` in schema.prisma and update `DATABASE_URL` accordingly.

2. **WebSocket Support**: The custom server.ts is required because Next.js App Router doesn't natively support WebSocket upgrades. This is a common pattern for real-time features.

3. **Backward Compatibility**: The GraphQL schema matches the Rust server exactly, so the Rust agent will work without modification (only the URL needs to change).

4. **Performance**: The in-memory state cache pattern from the Rust server is preserved for fast reads, with async database persistence.
