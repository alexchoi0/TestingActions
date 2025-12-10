import { createYoga } from 'graphql-yoga'
import { schema } from '@/lib/graphql/schema'
import { createLoaders, type DataLoaders } from '@/lib/graphql/loaders'

export interface GraphQLContext {
  loaders: DataLoaders
}

const yoga = createYoga({
  schema,
  graphqlEndpoint: '/graphql',
  fetchAPI: { Response },
  context: (): GraphQLContext => ({
    loaders: createLoaders()
  })
})

export const GET = yoga
export const POST = yoga
