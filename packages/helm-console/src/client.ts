import {
  realtimeEventFromSseFrame,
  streamFetchSse,
  type SseFrame,
} from '@reflective/helm-flow/realtime'
import type {
  ConsoleAdapter,
  ConsoleCommandDescriptor,
  ConsoleCommandResult,
  ConsoleConnection,
  ConsoleEvent,
  ConsoleReadDescriptor,
  ConsoleStreamDescriptor,
} from './types'

export class HelmConsoleClient {
  readonly adapter: ConsoleAdapter
  readonly baseUrl: string
  readonly bearerToken: string

  constructor(adapter: ConsoleAdapter, connection: Partial<ConsoleConnection> = {}) {
    this.adapter = adapter
    this.baseUrl = connection.baseUrl ?? adapter.connection.defaultBaseUrl
    this.bearerToken = connection.bearerToken ?? adapter.connection.localBearer ?? ''
  }

  async read<T = unknown>(
    descriptor: ConsoleReadDescriptor,
    params: Record<string, string | number | boolean | null | undefined> = {},
  ): Promise<T> {
    return this.request<T>(renderPath(descriptor.path, params), {
      method: descriptor.method ?? 'GET',
    })
  }

  async command<T = unknown>(
    command: ConsoleCommandDescriptor,
    values: Record<string, unknown> = {},
  ): Promise<ConsoleCommandResult<T>> {
    const request = command.request
    const path = renderPath(request.path, values)
    const url = withQuery(path, renderRecord(request.query, values))
    const body = renderRecord(request.body, values)
    const response = await this.request<T>(url, {
      method: request.method,
      body: request.method === 'GET' || body === undefined ? undefined : JSON.stringify(body),
    })

    return {
      command,
      response,
      authority: command.authority,
      expectedEventTypes: command.expectedEventTypes ?? [],
    }
  }

  async *stream(
    descriptor: ConsoleStreamDescriptor,
    params: Record<string, string | number | boolean | null | undefined> = {},
    init: RequestInit = {},
  ): AsyncGenerator<ConsoleEvent> {
    if (descriptor.transport !== 'sse') {
      throw new Error(`HelmConsoleClient only implements SSE streams today, got ${descriptor.transport}`)
    }

    const url = joinUrl(this.baseUrl, renderPath(descriptor.path, params))
    const headers = this.headers(init.headers)
    headers.delete('Content-Type')

    yield* streamFetchSse<ConsoleEvent>(url, {
      ...init,
      headers,
      parseEvent: consoleEventFromSseFrame,
    })
  }

  async request<T = unknown>(path: string, init: RequestInit = {}): Promise<T> {
    const response = await fetch(joinUrl(this.baseUrl, path), {
      ...init,
      headers: this.headers(init.headers),
    })

    if (!response.ok) {
      const detail = await response.text()
      throw new Error(`${response.status} ${response.statusText}${detail ? `: ${detail}` : ''}`)
    }

    if (response.status === 204) {
      return undefined as T
    }

    const contentType = response.headers.get('content-type') ?? ''
    if (!contentType.includes('application/json')) {
      return (await response.text()) as T
    }

    return (await response.json()) as T
  }

  private headers(extra?: HeadersInit): Headers {
    const headers = new Headers(extra)
    headers.set('Accept', 'application/json')
    if (!headers.has('Content-Type')) {
      headers.set('Content-Type', 'application/json')
    }
    if (this.bearerToken) {
      headers.set('Authorization', `Bearer ${this.bearerToken}`)
    }
    return headers
  }
}

export function joinUrl(baseUrl: string, path: string): string {
  const cleanBase = (baseUrl || '').endsWith('/') ? baseUrl.slice(0, -1) : baseUrl
  const cleanPath = path.startsWith('/') ? path : `/${path}`
  return `${cleanBase}${cleanPath}`
}

export function renderPath(
  path: string,
  values: Record<string, string | number | boolean | null | undefined | unknown>,
): string {
  return path.replace(/\{([^}]+)\}/g, (_match, key: string) => {
    const value = values[key]
    if (value === undefined || value === null || value === '') {
      throw new Error(`Missing path value: ${key}`)
    }
    return encodeURIComponent(String(value))
  })
}

export function withQuery(path: string, query?: Record<string, unknown>): string {
  if (!query) return path

  const params = new URLSearchParams()
  for (const [key, value] of Object.entries(query)) {
    if (value === undefined || value === null || value === '') continue
    params.set(key, String(value))
  }

  const rendered = params.toString()
  if (!rendered) return path
  return `${path}${path.includes('?') ? '&' : '?'}${rendered}`
}

function renderRecord(
  template: Record<string, unknown> | undefined,
  values: Record<string, unknown>,
): Record<string, unknown> | undefined {
  if (!template) return undefined
  return mapValue(template, values) as Record<string, unknown>
}

function mapValue(value: unknown, values: Record<string, unknown>): unknown {
  if (typeof value === 'string') {
    const match = value.match(/^\{([^}]+)\}$/)
    if (match) return values[match[1]]
    return value.replace(/\{([^}]+)\}/g, (_m, key: string) => String(values[key] ?? ''))
  }

  if (Array.isArray(value)) {
    return value.map((item) => mapValue(item, values))
  }

  if (value && typeof value === 'object') {
    return Object.fromEntries(
      Object.entries(value as Record<string, unknown>).map(([key, item]) => [key, mapValue(item, values)]),
    )
  }

  return value
}

function consoleEventFromSseFrame(frame: SseFrame): ConsoleEvent | null {
  try {
    const event = realtimeEventFromSseFrame(frame)
    return {
      ...event,
      raw: frame,
    }
  } catch {
    return {
      event_id: frame.id,
      type: frame.event ?? 'unparseable',
      payload: frame.data,
      raw: frame,
    }
  }
}
