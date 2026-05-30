import type { RealtimeEvent } from '@reflective/helm-flow/realtime'

export type ConsoleAuthority =
  | 'read-only'
  | 'chain-recorded'
  | 'receipt-bearing'
  | 'derived-recompute'

export type ConsoleMethod = 'GET' | 'POST' | 'PUT' | 'PATCH' | 'DELETE'

export type ConsoleFieldKind =
  | 'text'
  | 'textarea'
  | 'number'
  | 'password'
  | 'select'
  | 'checkbox'
  | 'json'
  | 'string-list'

export interface ConsoleNouns {
  run: string
  spec: string
  event: string
  artifact: string
}

export interface ConsoleConnectionDefaults {
  defaultBaseUrl: string
  localBearer?: string
}

export interface ConsoleFieldDescriptor {
  id: string
  label: string
  kind: ConsoleFieldKind
  required?: boolean
  placeholder?: string
  defaultValue?: unknown
  options?: Array<{ label: string; value: string }>
  help?: string
}

export interface ConsoleFormDescriptor {
  fields: ConsoleFieldDescriptor[]
}

export interface ConsoleRequestTemplate {
  method: ConsoleMethod
  path: string
  body?: Record<string, unknown>
  query?: Record<string, unknown>
}

export interface ConsoleReadDescriptor {
  id: string
  label: string
  path: string
  method?: Extract<ConsoleMethod, 'GET'>
  description?: string
}

export interface ConsoleStreamDescriptor {
  id: string
  label: string
  path: string
  transport: 'sse' | 'websocket' | 'tauri' | 'grpc'
  description?: string
}

export interface ConsoleCommandDescriptor {
  id: string
  label: string
  description?: string
  request: ConsoleRequestTemplate
  authority: Exclude<ConsoleAuthority, 'read-only'>
  form?: ConsoleFormDescriptor
  expectedEventTypes?: string[]
  redFlags?: string[]
}

export interface ConsoleControlGroup {
  id: string
  label: string
  commands: ConsoleCommandDescriptor[]
}

export interface ConsoleAidDescriptor {
  id: string
  label: string
  read?: ConsoleReadDescriptor
  recompute?: ConsoleCommandDescriptor
  aidKind: 'analytic' | 'memory' | 'solver' | 'policy' | 'prediction' | 'provider'
  authorityBoundary: string
}

export interface ConsoleArtifactDescriptor {
  id: string
  label: string
  read: ConsoleReadDescriptor
  resolverScheme?: string
  requiredProvenance?: string[]
}

export interface ConsoleSafetyRule {
  id: string
  label: string
  description: string
}

export interface ConsoleRunDescriptors {
  open?: ConsoleCommandDescriptor
  load: ConsoleReadDescriptor
  events?: ConsoleReadDescriptor
  live?: ConsoleStreamDescriptor
  integrity?: ConsoleReadDescriptor
  receipt?: ConsoleReadDescriptor
  outcome?: ConsoleReadDescriptor
}

export interface ConsoleAdapter {
  appId: string
  displayName: string
  routePrefix: string
  subjectRefKind: string
  nouns: ConsoleNouns
  connection: ConsoleConnectionDefaults
  run: ConsoleRunDescriptors
  controls: ConsoleControlGroup[]
  aids: ConsoleAidDescriptor[]
  artifacts: ConsoleArtifactDescriptor[]
  safety: ConsoleSafetyRule[]
}

export interface ConsoleConnection {
  baseUrl: string
  bearerToken?: string
}

export interface ConsoleCommandResult<T = unknown> {
  command: ConsoleCommandDescriptor
  response: T
  authority: ConsoleAuthority
  expectedEventTypes: string[]
}

export interface ConsoleEvent<TPayload = unknown> extends RealtimeEvent<TPayload> {
  raw?: unknown
}

export interface ProofArtifactSummary {
  id: string
  label: string
  status?: string
  contentHash?: string
  generatedAt?: string
  unresolvedCount?: number
  summary?: string
}

export interface ConsoleRedFlagFinding {
  ruleId: string
  message: string
  severity: 'info' | 'warning' | 'fail'
}
