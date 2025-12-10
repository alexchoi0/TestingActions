import { createServer } from 'http'
import { parse } from 'url'
import next from 'next'
import { WebSocketServer } from 'ws'
import { useServer } from 'graphql-ws/use/ws'
import { schema } from './src/lib/graphql/schema'

const dev = process.env.NODE_ENV !== 'production'
const hostname = 'localhost'
const port = parseInt(process.env.PORT || '3000', 10)

const app = next({ dev, hostname, port })
const handle = app.getRequestHandler()

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
    console.log(`> GraphQL endpoint: http://${hostname}:${port}/graphql`)
    console.log(`> WebSocket subscriptions: ws://${hostname}:${port}/graphql/ws`)
  })
})
