/**
 * Helm realtime primitives.
 *
 * These utilities keep UI code independent from the transport that carries
 * workflow events. The first adapter is fetch-based SSE because Catalyst and
 * other job surfaces start streams with POST bodies, which EventSource cannot do.
 */

export type RealtimeTransport = 'grpc' | 'sse' | 'websocket' | 'tauri' | 'polling'

export interface RealtimeActor {
  type: 'human' | 'agent' | 'system' | 'external'
  id: string
  display_name?: string
}

export interface RealtimeEvent<TPayload = unknown, TType extends string = string> {
  event_id?: string
  sequence?: number
  type: TType
  schema_version?: number
  occurred_at?: string
  app_id?: string
  run_id?: string
  job_id?: string
  correlation_id?: string
  actor?: RealtimeActor
  payload: TPayload
}

export interface RealtimeResumeCursor {
  sequence?: number
  last_event_id?: string
}

export interface SseFrame {
  event?: string
  id?: string
  retry?: number
  data: string
}

export interface FetchSseOptions<TEvent> extends RequestInit {
  parseEvent?: (frame: SseFrame) => TEvent | null | undefined
}

export function parseJsonSseFrame(frame: SseFrame): unknown {
  return frame.data ? JSON.parse(frame.data) : null
}

export function cursorFromRealtimeEvent(event: RealtimeEvent): RealtimeResumeCursor {
  return {
    sequence: event.sequence,
    last_event_id: event.event_id,
  }
}

export function withResumeQuery(
  url: string,
  cursor: RealtimeResumeCursor,
  sequenceParam = 'since_seq',
  eventIdParam = 'last_event_id',
): string {
  if (cursor.sequence === undefined && !cursor.last_event_id) return url

  const parsed = new URL(url, 'http://localhost')
  if (cursor.sequence !== undefined) parsed.searchParams.set(sequenceParam, String(cursor.sequence))
  if (cursor.last_event_id) parsed.searchParams.set(eventIdParam, cursor.last_event_id)

  const rendered = parsed.toString()
  return url.startsWith('http://') || url.startsWith('https://') ? rendered : rendered.replace(parsed.origin, '')
}

export function realtimeEventFromSseFrame(frame: SseFrame): RealtimeEvent {
  const data = parseJsonSseFrame(frame)
  const record = asRecord(data)

  if (!record) {
    return {
      event_id: frame.id,
      sequence: sequenceFromFrameId(frame.id),
      type: frame.event ?? 'message',
      payload: data,
    }
  }

  const payload = record.payload ?? data

  return {
    event_id: stringField(record, 'event_id') ?? stringField(record, 'eventId') ?? frame.id,
    sequence: numberField(record, 'sequence') ?? sequenceFromFrameId(frame.id),
    type: stringField(record, 'type') ?? stringField(record, 'event') ?? frame.event ?? 'message',
    schema_version: numberField(record, 'schema_version') ?? numberField(record, 'schemaVersion'),
    occurred_at: stringField(record, 'occurred_at') ?? stringField(record, 'occurredAt'),
    app_id: stringField(record, 'app_id') ?? stringField(record, 'appId'),
    run_id: stringField(record, 'run_id') ?? stringField(record, 'runId'),
    job_id: stringField(record, 'job_id') ?? stringField(record, 'jobId'),
    correlation_id: stringField(record, 'correlation_id') ?? stringField(record, 'correlationId'),
    actor: actorField(record.actor),
    payload,
  }
}

export async function* streamFetchSse<TEvent = SseFrame>(
  url: string,
  options: FetchSseOptions<TEvent> = {},
): AsyncGenerator<TEvent> {
  const { parseEvent, ...init } = options
  const response = await fetch(url, init)

  if (!response.ok) {
    const body = await response.text()
    throw new Error(body || `SSE stream failed with ${response.status}`)
  }

  const reader = response.body?.getReader()
  if (!reader) throw new Error('SSE stream has no response body')

  const decoder = new TextDecoder()
  let buffer = ''
  let pendingEvent: string | undefined
  let pendingId: string | undefined
  let pendingRetry: number | undefined
  let pendingData: string[] = []

  const flushFrame = (): SseFrame | null => {
    if (pendingData.length === 0) {
      pendingEvent = undefined
      pendingRetry = undefined
      return null
    }

    const frame: SseFrame = {
      event: pendingEvent,
      id: pendingId,
      retry: pendingRetry,
      data: pendingData.join('\n'),
    }

    pendingEvent = undefined
    pendingRetry = undefined
    pendingData = []
    return frame
  }

  const readLine = (line: string): SseFrame | null => {
    if (line === '') return flushFrame()
    if (line.startsWith(':')) return null

    const separator = line.indexOf(':')
    const field = separator >= 0 ? line.slice(0, separator) : line
    const value = separator >= 0 ? line.slice(separator + 1).replace(/^ /, '') : ''

    switch (field) {
      case 'event':
        pendingEvent = value
        break
      case 'data':
        pendingData.push(value)
        break
      case 'id':
        pendingId = value
        break
      case 'retry':
        pendingRetry = Number.parseInt(value, 10)
        if (Number.isNaN(pendingRetry)) pendingRetry = undefined
        break
    }

    return null
  }

  try {
    while (true) {
      const { done, value } = await reader.read()
      if (done) break

      buffer += decoder.decode(value, { stream: true })
      const lines = buffer.split(/\r?\n/)
      buffer = lines.pop() ?? ''

      for (const line of lines) {
        const frame = readLine(line)
        if (!frame) continue

        const event = parseEvent ? parseEvent(frame) : (frame as TEvent)
        if (event !== null && event !== undefined) yield event
      }
    }

    buffer += decoder.decode()
    if (buffer) {
      const frame = readLine(buffer)
      if (frame) {
        const event = parseEvent ? parseEvent(frame) : (frame as TEvent)
        if (event !== null && event !== undefined) yield event
      }
    }

    const finalFrame = flushFrame()
    if (finalFrame) {
      const event = parseEvent ? parseEvent(finalFrame) : (finalFrame as TEvent)
      if (event !== null && event !== undefined) yield event
    }
  } finally {
    reader.releaseLock()
  }
}

function asRecord(value: unknown): Record<string, unknown> | null {
  return value && typeof value === 'object' && !Array.isArray(value) ? (value as Record<string, unknown>) : null
}

function stringField(record: Record<string, unknown>, key: string): string | undefined {
  const value = record[key]
  return typeof value === 'string' ? value : undefined
}

function numberField(record: Record<string, unknown>, key: string): number | undefined {
  const value = record[key]
  return typeof value === 'number' ? value : undefined
}

function sequenceFromFrameId(id: string | undefined): number | undefined {
  if (!id) return undefined
  const sequence = Number.parseInt(id, 10)
  return Number.isNaN(sequence) ? undefined : sequence
}

function actorField(value: unknown): RealtimeActor | undefined {
  const record = asRecord(value)
  if (!record) return undefined

  const actorType = stringField(record, 'type')
  const id = stringField(record, 'id')
  if (!actorType || !id) return undefined
  if (actorType !== 'human' && actorType !== 'agent' && actorType !== 'system' && actorType !== 'external') {
    return undefined
  }

  return {
    type: actorType,
    id,
    display_name: stringField(record, 'display_name') ?? stringField(record, 'displayName'),
  }
}
