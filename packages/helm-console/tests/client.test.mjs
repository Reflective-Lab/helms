import assert from 'node:assert/strict'
import test from 'node:test'

import { joinUrl, renderPath, withQuery } from '../dist/client.js'

test('console client helpers render app-supplied paths without built-in profiles', () => {
  assert.equal(joinUrl('/app', '/runs/123'), '/app/runs/123')
  assert.equal(renderPath('/runs/{id}/events', { id: 'run 123' }), '/runs/run%20123/events')
  assert.equal(withQuery('/runs', { cursor: 'next', empty: '', skipped: null }), '/runs?cursor=next')
})
