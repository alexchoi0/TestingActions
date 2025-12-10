'use client'

import { ApolloClient, InMemoryCache, split, HttpLink } from '@apollo/client/core'
import { ApolloProvider } from '@apollo/client/react'
import { GraphQLWsLink } from '@apollo/client/link/subscriptions'
import { getMainDefinition } from '@apollo/client/utilities'
import { createClient } from 'graphql-ws'
import { ReactNode } from 'react'

const httpLink = new HttpLink({
  uri: '/graphql'
})

const wsLink = typeof window !== 'undefined'
  ? new GraphQLWsLink(createClient({
      url: `ws://${window.location.host}/graphql/ws`,
      retryAttempts: 0,
      shouldRetry: () => false,
      lazy: true,
      on: {
        error: () => {},
        closed: () => {},
      },
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

export function GraphQLProvider({ children }: { children: ReactNode }) {
  return <ApolloProvider client={client}>{children}</ApolloProvider>
}
