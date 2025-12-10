'use client'

import { gql } from '@apollo/client/core'
import { useQuery, useSubscription, useMutation } from '@apollo/client/react'

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

const RUN_QUERY = gql`
  query Run($id: ID!) {
    run(id: $id) {
      id
      status
      workflowsDir
      startedAt
      completedAt
      eventCount
    }
  }
`

const RUN_EVENTS_QUERY = gql`
  query RunEvents($runId: ID!) {
    runEvents(runId: $runId) {
      eventType
      runId
      timestamp
      workflowName
      jobName
      stepIndex
      stepName
      success
      error
      reason
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
      reason
    }
  }
`

const EVENTS_FOR_RUN_SUBSCRIPTION = gql`
  subscription EventsForRun($runId: ID!) {
    eventsForRun(runId: $runId) {
      eventType
      runId
      timestamp
      workflowName
      jobName
      stepIndex
      stepName
      success
      error
      reason
    }
  }
`

const STOP_RUN_MUTATION = gql`
  mutation StopRun($runId: ID!) {
    stopRun(runId: $runId)
  }
`

export interface Run {
  id: string
  status: string
  workflowsDir: string
  startedAt: string
  completedAt: string | null
  eventCount: number
}

export interface RunEvent {
  eventType: string
  runId: string
  timestamp: string
  workflowName: string | null
  jobName: string | null
  stepIndex: number | null
  stepName: string | null
  success: boolean | null
  error: string | null
  reason: string | null
}

export function useRuns(limit = 20, offset = 0) {
  const { data, loading, error, refetch } = useQuery<{ runs: Run[] }>(RUNS_QUERY, {
    variables: { limit, offset }
  })

  useSubscription(EVENTS_SUBSCRIPTION, {
    onData: () => refetch()
  })

  return { runs: data?.runs ?? [], loading, error, refetch }
}

export function useRun(id: string) {
  const { data, loading, error, refetch } = useQuery<{ run: Run | null }>(RUN_QUERY, {
    variables: { id },
    skip: !id
  })

  useSubscription(EVENTS_FOR_RUN_SUBSCRIPTION, {
    variables: { runId: id },
    skip: !id,
    onData: () => refetch()
  })

  return { run: data?.run ?? null, loading, error, refetch }
}

export function useRunEvents(runId: string) {
  const { data, loading, error, refetch } = useQuery<{ runEvents: RunEvent[] }>(RUN_EVENTS_QUERY, {
    variables: { runId },
    skip: !runId
  })

  useSubscription(EVENTS_FOR_RUN_SUBSCRIPTION, {
    variables: { runId },
    skip: !runId,
    onData: () => refetch()
  })

  return { events: data?.runEvents ?? [], loading, error, refetch }
}

export function useStopRun() {
  const [stopRunMutation, { loading, error }] = useMutation<{ stopRun: boolean }>(STOP_RUN_MUTATION)

  const stopRun = async (runId: string) => {
    const result = await stopRunMutation({ variables: { runId } })
    return result.data?.stopRun ?? false
  }

  return { stopRun, loading, error }
}
