import { createYoga } from 'graphql-yoga'
import { schema } from '@/lib/graphql/schema'

const yoga = createYoga({
  schema,
  graphqlEndpoint: '/graphql',
  fetchAPI: { Response }
})

export { yoga as GET, yoga as POST }
